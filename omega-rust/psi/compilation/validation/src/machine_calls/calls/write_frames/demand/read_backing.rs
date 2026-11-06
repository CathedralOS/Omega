//! Frozen dependencies of one admitted storage read.
//!
//! Typed suppliers and descriptor slots retain their storage namespaces.
//! Presentation paths only retire existing observations; they never establish
//! a new value, lifetime, loan or reference origin.

use super::super::{caller_aliases, reference_subjects, state_write_walk, stored_calls};
use super::{
    CallFrameResolver, CollectedStatementPrefix, ExpressionHandle, FrameInference, Machine, State,
};
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CapturedStorageReadBacking {
    pub(crate) sources: Vec<super::super::LocalWriteSource>,
    pub(crate) values: Vec<String>,
    pub(crate) bindings: Vec<super::super::LocalWriteSource>,
}

impl CallFrameResolver<'_> {
    /// Contents are observed through this exact completed value's descriptor.
    /// Suppliers and selectors come from its performed evaluation; another
    /// pointer in the suffix uses the captured post-evaluation inventory.
    pub(crate) fn completed_value_read_backing(
        &self,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        reference: typed_trees::types::TypeReferenceHandle,
        path: &[facts::PlaceSegment],
    ) -> Option<CapturedStorageReadBacking> {
        let (owner, statement, index) = caller_aliases::caller_statement_owner(
            self.program,
            machine,
            caller_aliases::CallerWriteSite::Expression(expression),
        )?;
        (owner.symbol == state.symbol).then_some(())?;
        // Contextual constructors already completed under this carrier. Their
        // unused parameters cannot be recovered from the payload alone.
        let (_, completed) =
            self.completed_read_value_at(machine, owner, statement, index, expression, reference)?;
        completed.read_backing(self.program, path)
    }

    /// Complete supplier frontier retained by an observed reference value. Its
    /// coarse mutation footprints are not equations with any one supplier.
    /// Immutable names
    /// and saved subjects select their authenticated production cut, even when
    /// normalization reuses their syntax in several later statements. Neither
    /// mutable selectors nor subsequently loaded pointers are replayed there.
    pub(crate) fn completed_reference_read_sources(
        &self,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<Vec<super::super::LocalWriteSource>> {
        let (owner, completed) = match self.program.expression_table.expression(expression) {
            typed_trees::expression::ExpressionNode::Name(path) => {
                if let Some((index, local)) =
                    self.program.state_local_declaration(state, path.symbol)
                    && !local.is_mutable
                    && path.symbol == path.head_symbol
                {
                    let initial = local.ordinary_value_initializer()?;
                    let statement = self
                        .program
                        .statement_table
                        .statements(state.statement_nodes)
                        .get(index)?;
                    let reference = crate::expression_result_type_reference(
                        self.program,
                        machine,
                        state,
                        initial,
                    )?;
                    self.completed_read_value_at(
                        machine, state, statement, index, initial, reference,
                    )?
                } else {
                    self.completed_read_value(machine, expression)?
                }
            }
            typed_trees::expression::ExpressionNode::SavedSubject(saved) => {
                let production = self.program.saved_value_source(saved)?;
                self.completed_read_value(machine, production)?
            }
            _ => self.completed_read_value(machine, expression)?,
        };
        if owner != state.symbol {
            return None;
        }
        let sources = completed.reference_sources(self.program);
        if sources.is_none() && std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
            eprintln!(
                "STRUCT captured-reference-address-refused machine={} expression={expression:?} phase=descriptor",
                self.program.symbols.name(machine.symbol)
            );
        }
        sources
    }

    /// Track possible recast phases on the exact completed operand. Precision
    /// sufficient to read pointee contents is independently required by the
    /// storage-read APIs; it cannot erase a coarse descriptor's obligations.
    pub(crate) fn completed_reference_interpretation_sources(
        &self,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Result<Vec<super::super::LocalWriteSource>, &'static str> {
        let (owner, completed) = self
            .completed_read_value(machine, expression)
            .ok_or("reference operand has no complete ordered capture")?;
        if owner != state.symbol {
            return Err("reference capture belongs to a different state");
        }
        completed.interpretation_sources(self.program)
    }

    fn completed_read_value(
        &self,
        machine: &Machine,
        expression: ExpressionHandle,
    ) -> Option<(SymbolHandle, stored_calls::value_reads::CompletedValueReads)> {
        let (state, statement, index) = caller_aliases::caller_statement_owner(
            self.program,
            machine,
            caller_aliases::CallerWriteSite::Expression(expression),
        )?;
        let reference =
            crate::expression_result_type_reference(self.program, machine, state, expression)?;
        self.completed_read_value_at(machine, state, statement, index, expression, reference)
    }

    fn completed_read_value_at(
        &self,
        machine: &Machine,
        state: &State,
        statement: &typed_trees::statement::StatementNode,
        index: usize,
        expression: ExpressionHandle,
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> Option<(SymbolHandle, stored_calls::value_reads::CompletedValueReads)> {
        let prefix = state_write_walk::collected_prefix_at(
            &self.caches.state_write_shared_collections,
            self.program,
            machine,
            state,
            &self.tables.symbols,
            index,
            true,
        )?;
        Some((
            state.symbol,
            stored_calls::completed_value_reads(
                self.program,
                machine,
                state,
                statement,
                expression,
                reference,
                &prefix,
                &self.tables.symbols,
            )?,
        ))
    }

    /// Invalidation paths of fields reached through one completed reference
    /// value. The copied descriptor's old home is frozen; later pointer slots
    /// and pointee contents remain live dependencies. Every path keeps its own
    /// unavailable result, and all alternatives must supply complete backing.
    /// These paths grant no value equality, read, loan or result guarantee.
    pub fn captured_reference_read_paths(
        &self,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        paths: &[Vec<facts::PlaceSegment>],
    ) -> Vec<Option<Vec<String>>> {
        let completed = (|| {
            let reference =
                crate::expression_result_type_reference(self.program, machine, state, expression)?;
            let shell = crate::reference_shell(self.program, reference)?;
            if !shell.access.is_readable() {
                return None;
            }
            let (owner, completed) = self.completed_read_value(machine, expression)?;
            (owner == state.symbol).then_some(completed)
        })();
        if completed.is_none() && std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
            eprintln!(
                "STRUCT captured-reference-dependency-refused machine={:?} expression={expression:?} phase=completion",
                machine.symbol
            );
        }
        paths
            .iter()
            .map(|path| {
                let result = completed.as_ref()?.invalidation_paths(self.program, path);
                if result.is_none() && std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
                    eprintln!("STRUCT captured-reference-dependency-refused machine={:?} expression={expression:?} path={path:?} phase=projection", machine.symbol);
                }
                result
            })
            .collect()
    }

    /// An invocation-entry contract observes this original statement prefix.
    pub(crate) fn storage_read_backing_before_statement(
        &self,
        machine: &Machine,
        state: &State,
        statement: usize,
        root: SymbolHandle,
        path: &[facts::PlaceSegment],
    ) -> Option<CapturedStorageReadBacking> {
        let prefix = state_write_walk::collected_prefix_at(
            &self.caches.state_write_shared_collections,
            self.program,
            machine,
            state,
            &self.tables.symbols,
            statement,
            true,
        )?;
        reference_subjects::read_backing::capture(
            self.program,
            machine,
            state,
            statement,
            root,
            path,
            &prefix,
        )
    }

    /// Classify source substitution for this exact completed scalar read.
    /// Numerical premises on its backing require the selected interpretation;
    /// captured-value proofs handle a different representation separately.
    pub fn scalar_read_changes_interpretation(
        &self,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        root: SymbolHandle,
        path: &[facts::PlaceSegment],
    ) -> Option<bool> {
        let reference =
            crate::expression_result_type_reference(self.program, machine, state, expression)?;
        let (expected, _) = crate::value_custody::expression_types::scalar_observation_meaning(
            self.program,
            reference,
        )?;
        let backing =
            self.storage_read_backing_at_expression(machine, state, expression, root, path)?;
        Some(backing.sources.iter().any(|source| {
            source
                .scalar_storage_coordinate(self.program, state)
                .map_or_else(
                    || !source.recasts.is_empty() || source.representation.is_some(),
                    |scalar| {
                        !scalar.preserves_scalar_value
                            || scalar.source_primitive != expected
                            || scalar.view_primitive != expected
                    },
                )
        }))
    }

    /// A completed read follows its evaluated children. Earlier operands may
    /// replace a descriptor, so its incoming statement prefix is insufficient.
    pub(crate) fn storage_read_backing_at_expression(
        &self,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        root: SymbolHandle,
        path: &[facts::PlaceSegment],
    ) -> Option<CapturedStorageReadBacking> {
        let (owner, statement, index) = caller_aliases::caller_statement_owner(
            self.program,
            machine,
            caller_aliases::CallerWriteSite::Expression(expression),
        )?;
        if owner.symbol != state.symbol {
            return None;
        }
        let prefix = state_write_walk::collected_prefix_at(
            &self.caches.state_write_shared_collections,
            self.program,
            machine,
            state,
            &self.tables.symbols,
            index,
            true,
        )?;
        let transfer = stored_calls::effects_at_expression(
            self.program,
            machine,
            state,
            statement,
            &prefix,
            &self.tables.symbols,
            &mut FrameInference::for_state(state.symbol),
            expression,
        )?;
        if !transfer.is_complete() {
            return None;
        }
        let mut after_children: CollectedStatementPrefix = (*prefix).clone();
        stored_calls::apply_transfer(
            self.program,
            &mut after_children.stored,
            &mut after_children.aliases,
            &mut after_children.divergent,
            &transfer,
        );
        reference_subjects::read_backing::capture(
            self.program,
            machine,
            state,
            index,
            root,
            path,
            &after_children,
        )
    }
}
