//! Construct and retain the typed forest.
use crate::source_symbols::SymbolTable;
use crate::*;
use std::ops::{Deref, DerefMut};
impl Clone for TypedTrees {
    fn clone(&self) -> Self {
        self.clone_with_symbols(self.symbols.clone())
    }
}

impl TypedTrees {
    /// Install the continuation's symbols without copying the old symbol table
    /// that they replace. Other storage follows its owner's clone contract.
    /// The caller must establish the replacement's source authority first.
    pub fn with_replaced_symbols(
        mut self: std::sync::Arc<Self>,
        symbols: SymbolTable,
    ) -> std::sync::Arc<Self> {
        if let Some(program) = std::sync::Arc::get_mut(&mut self) {
            program.symbols = symbols;
            return self;
        }
        let program = match std::sync::Arc::try_unwrap(self) {
            Ok(mut program) => {
                program.symbols = symbols;
                program
            }
            Err(program) => program.clone_with_symbols(symbols),
        };
        std::sync::Arc::new(program)
    }

    fn clone_with_symbols(&self, symbols: SymbolTable) -> Self {
        macro_rules! clone_storage {
            ($($field:ident),+ $(,)?) => {{
                let Self { identity, symbols: _, $($field,)+ } = self;
                Self { identity: *identity, symbols, $($field: $field.clone(),)+ }
            }};
        }
        clone_storage!(
            build_invocation_entry,
            excluded_authored_selection_occurrences,
            roots,
            tables,
            service_reaches,
            service_reach_rows,
            authored_service_reach_rows,
            semantic_domains,
            applied_domain_applications,
            applied_data_cleanup_selections,
            opaque_representation_selections,
            external_bindings,
            plan_laid_layouts,
            plan_laid_target_layouts,
            plan_laid_schema_fields,
            plan_laid_retired_field_identities,
            access_view_plans,
            access_lifecycle_plans,
            access_policy_invocations,
            access_borrow_factories,
            wire_placements,
            wire_encode_obligations,
            wire_schema_plans,
            machine_specializations,
            captured_machine_applications,
            captured_machine_states,
            trait_implementation_applications,
            data_implementation_applications,
            target_machine_families,
            deferred_refusals,
            evidence_forwardings,
            proof_output_calls,
            declaration_indexes,
            ranking_expression_custody,
            pending_const_range_endpoints,
            constant_recipes,
            constant_expression_contexts,
            constant_recipe_closure,
            callable_identities,
            type_identities,
        )
    }
}

impl TypedTrees {
    pub fn machine_specialization(
        &self,
        instance: symbols::SymbolHandle,
    ) -> Option<&MachineSpecialization> {
        match self.machine_instantiation(instance).ok()?? {
            callable_applications::MachineInstantiation::Specialization(application) => {
                Some(application)
            }
            callable_applications::MachineInstantiation::Capture(_, _) => None,
        }
    }

    /*
        The core vocabulary row of `symbol`'s authored declaration. A machine
        specialization keeps its template's source span but lives under a
        generated namespace, so it is resolved to the template first; the
        catalog then judges the authored declaration's file, path and bytes.
    */
    pub fn core_vocabulary(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Option<language_core::core_vocabulary::CoreVocabulary> {
        let original = self
            .machine_specialization(symbol)
            .map_or(symbol, |specialization| specialization.template);
        self.symbols.core_vocabulary(original)
    }

    pub fn checks_authored_selection(
        &self,
        selection: &crate::source_selections::AuthoredDeclarationSelection,
    ) -> bool {
        !self
            .excluded_authored_selection_occurrences
            .contains(selection.occurrence_id())

    }

    pub fn authored_service_reach_rows_for(
        &self,
        owner: symbols::SymbolHandle,
    ) -> impl Iterator<Item = &signature::AuthoredServiceReachRow> {
        self.authored_service_reach_rows
            .iter()
            .filter(move |row| row.owner == owner)
    }

    pub fn with_roots(
        roots: TypedTreeRoots,
        tables: TypedTreeTables,
        symbols: SymbolTable,
    ) -> Self {
        Self {
            identity: ProgramIdentity::default(),
            build_invocation_entry: None,
            excluded_authored_selection_occurrences: AuthoredSelectionExclusions::default(),
            roots,
            tables,
            symbols,
            service_reaches: crate::source_tables::ServiceReachTable::default(),
            service_reach_rows: crate::source_tables::ServiceReachRowTable::default(),
            authored_service_reach_rows: Default::default(),
            semantic_domains: crate::source_tables::SemanticDomainTable::default(),
            applied_domain_applications: Default::default(),
            applied_data_cleanup_selections: Vec::new(),
            opaque_representation_selections: Vec::new(),
            external_bindings: crate::source_tables::ExternalBindingTable::default(),
            plan_laid_layouts: Vec::new(),
            plan_laid_target_layouts: arena::RootedArena::new(),
            plan_laid_schema_fields: arena::RootedArena::new(),
            plan_laid_retired_field_identities: arena::RootedArena::new(),
            access_view_plans: Vec::new(),
            access_lifecycle_plans: arena::RootedArena::new(),
            access_policy_invocations: arena::RootedArena::new(),
            access_borrow_factories: Default::default(),
            wire_placements: arena::RootedArena::new(),
            wire_encode_obligations: arena::RootedArena::new(),
            wire_schema_plans: Vec::new(),
            machine_specializations: MachineSpecializations::default(),
            captured_machine_applications: arena::RootedArena::new(),
            captured_machine_states: arena::RootedArena::new(),
            trait_implementation_applications: arena::RootedArena::new(),
            data_implementation_applications: arena::RootedArena::new(),
            target_machine_families: Vec::new(),
            deferred_refusals: Default::default(),
            evidence_forwardings: Vec::new(),
            proof_output_calls: Vec::new(),
            declaration_indexes: declaration_index::DeclarationIndexes::default(),
            ranking_expression_custody: Default::default(),
            pending_const_range_endpoints: std::collections::HashSet::new(),
            constant_recipes: arena::RootedArena::new(),
            constant_expression_contexts: arena::RootedArena::new(),
            constant_recipe_closure: constant_recipes::ConstantRecipeClosure::default(),
            callable_identities: OwnedInterner::default(),
            type_identities: Default::default(),
        }
    }

    pub fn ranking_expression_custody_for(
        &self,
        machine: symbols::SymbolHandle,
    ) -> Option<&crate::source_state::ranking::RankingExpressionCustody> {
        self.ranking_expression_custody
            .iter()
            .find(|custody| custody.machine == machine)
    }
}

impl Deref for TypedTrees {
    type Target = TypedTreeTables;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.tables
    }
}

impl DerefMut for TypedTrees {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tables
    }
}
