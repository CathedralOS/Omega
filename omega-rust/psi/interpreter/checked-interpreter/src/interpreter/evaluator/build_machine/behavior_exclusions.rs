//! Executed Build behavior exclusions recorded as they evaluate: crash
//! causes, physical-authority classes, and abstract services.
//!
//! Exclusions are evaluated Build selections
//! (wiki/spec/build/behavior_exclusions.md): recording the call as it
//! executes — not scanning the spelled text — is what makes a statically
//! present but unexecuted call not a selection. The receiver must be the
//! activation's original Build cell, the same authority `roots.bind`
//! requires: a helper may assemble a selection only because the root Build
//! value reaches its frame. A call evaluated on any other receiver claims
//! an authority it does not have and traps rather than silently applying
//! or silently dropping.
use crate::interpreter::evaluator::{
    Cell, EvalResult, Evaluator, ExpressionHandle, Frame, Halt, SymbolHandle, TableCall, Value,
    trap,
};
use crate::{
    ExecutedBehaviorExclusion, ExecutedBehaviorExclusionKind, ExecutedBehaviorExclusionSite,
};
use language_semantics::declaration_selection::BuildOperation;
use typed_trees::statement::StatementHandle;

impl<'program> Evaluator<'program> {
    /// Service selections are parser-owned markers; enum selections are ordinary
    /// resolved static calls. Both record their executed source coordinate.
    pub(in crate::interpreter::evaluator) fn try_behavior_exclusion_statement(
        &mut self,
        statement: StatementHandle,
        call: &TableCall,
        frame: &mut Frame,
    ) -> EvalResult<Option<Value>> {
        let kind = if BuildOperation::from_call_target(call.target.as_str())
            == Some(BuildOperation::ServiceExclusion)
            && !call.target_symbol.is_valid()
        {
            self.require_root_build_receiver(
                self.statement_receiver_cell(call, frame)?,
                call.target.as_str(),
            )?;
            ExecutedBehaviorExclusionKind::Service
        } else {
            let arguments = self
                .program
                .statement_table
                .expression_handles(call.arguments);
            let Some(kind) = self.evaluate_enum_exclusion(call.target_symbol, arguments, frame)?
            else {
                return Ok(None);
            };
            kind
        };
        self.record_executed_exclusion(
            ExecutedBehaviorExclusionSite::Statement(statement),
            kind,
            frame,
        )?;
        Ok(Some(Value::Unit))
    }

    pub(in crate::interpreter::evaluator) fn try_behavior_exclusion_value_call(
        &mut self,
        handle: ExpressionHandle,
        call: &typed_trees::expression::TableCallExpression,
        frame: &mut Frame,
    ) -> EvalResult<Option<Value>> {
        let kind = if BuildOperation::from_call_target(call.target.as_str())
            == Some(BuildOperation::ServiceExclusion)
            && !call.target_symbol.is_valid()
        {
            let receiver = if call.receiver.is_valid() {
                self.resolve_place(call.receiver, frame)
                    .ok()
                    .map(|cell| self.deref_cell(cell))
            } else {
                None
            };
            self.require_root_build_receiver(receiver, call.target.as_str())?;
            ExecutedBehaviorExclusionKind::Service
        } else {
            let arguments = self
                .program
                .expression_table
                .expression_handles(call.arguments);
            let Some(kind) = self.evaluate_enum_exclusion(call.target_symbol, arguments, frame)?
            else {
                return Ok(None);
            };
            kind
        };
        self.record_executed_exclusion(
            ExecutedBehaviorExclusionSite::Expression(handle),
            kind,
            frame,
        )?;
        Ok(Some(Value::Unit))
    }

    /// Static Build operations carry their authority as the first argument,
    /// never in the declaration namespace. Select by exact resolved toolchain
    /// identity, evaluate arguments once in authored order, and retain the case.
    /// Executing the prelude's empty body would silently discard the restriction.
    fn evaluate_enum_exclusion(
        &mut self,
        target_symbol: SymbolHandle,
        arguments: &[ExpressionHandle],
        frame: &mut Frame,
    ) -> EvalResult<Option<ExecutedBehaviorExclusionKind>> {
        // Most interpreter calls are ordinary program calls. Reject those by
        // provenance before searching the compiler-owned facet declarations.
        if !target_symbol.is_valid() || !self.symbol_has_build_prelude_source(target_symbol) {
            return Ok(None);
        }
        let (method_name, type_name) =
            if self.exact_build_facet_method("Build", "exclude_crash", target_symbol) {
                ("exclude_crash", "CrashCause")
            } else if self.exact_build_facet_method(
                "Build",
                "exclude_physical_authority",
                target_symbol,
            ) {
                ("exclude_physical_authority", "PhysicalAuthorityClass")
            } else {
                return Ok(None);
            };
        let [builder, argument] = arguments else {
            return trap(format!(
                "behavior exclusion `{method_name}` takes a Build reference and one {type_name} value"
            ));
        };
        // Ordinary argument evaluation preserves reference forwarding; value
        // evaluation would read a copy and erase the activation's identity.
        let receiver = self.eval_argument(*builder, frame)?;
        self.require_root_build_receiver(Some(self.deref_cell(receiver)), method_name)?;
        let case_symbol =
            self.evaluated_exclusion_case(*argument, frame, method_name, type_name)?;
        Ok(Some(match type_name {
            "CrashCause" => ExecutedBehaviorExclusionKind::CrashCause { case_symbol },
            _ => ExecutedBehaviorExclusionKind::PhysicalAuthorityClass { case_symbol },
        }))
    }

    /// Recover an exact toolchain case from an ordinary evaluated value.
    /// Locals and computed cases select just as literal cases do. Both enum
    /// axes share the same identity check; names are interpreted only at this
    /// compiler-owned source boundary, never as physical classification.
    fn evaluated_exclusion_case(
        &mut self,
        argument: ExpressionHandle,
        frame: &mut Frame,
        method_name: &str,
        type_name: &str,
    ) -> EvalResult<SymbolHandle> {
        let Value::Enum {
            type_symbol,
            variant_name,
            payload,
        } = self.eval_expression(argument, frame)?
        else {
            return trap(format!(
                "behavior exclusion `{method_name}` argument did not evaluate to a compiler-owned {type_name} case"
            ));
        };
        let is_toolchain_case = type_symbol != SymbolHandle::invalid()
            && self.symbol_has_build_prelude_source(type_symbol)
            && self.program.data_definitions().iter().any(|definition| {
                definition.symbol == type_symbol && definition.name.as_str() == type_name
            });
        if !is_toolchain_case {
            return trap(format!(
                "behavior exclusion `{method_name}` argument did not evaluate to a compiler-owned {type_name} case"
            ));
        }
        if !payload.is_empty() {
            return trap(format!(
                "compiler-owned {type_name} case unexpectedly carries a payload"
            ));
        }
        let variant = self
            .program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == type_symbol)
            .and_then(|data| {
                self.program
                    .data_members(data)
                    .iter()
                    .find_map(|member| match member {
                        typed_trees::data::DataMember::Variant(variant)
                            if *variant.name.as_str() == *variant_name =>
                        {
                            Some(variant.symbol)
                        }
                        _ => None,
                    })
            });
        variant.ok_or_else(|| {
            Halt::Trap(format!(
                "evaluated {type_name} value names no declared compiler-owned case"
            ))
        })
    }

    /// The receiver must dereference to the activation's original Build cell
    /// — the same authority `execute_root_binding` requires.
    fn require_root_build_receiver(
        &self,
        receiver: Option<Cell>,
        spelling: &str,
    ) -> EvalResult<()> {
        if receiver
            .as_ref()
            .zip(self.root_build.as_ref())
            .is_some_and(|(receiver, root)| Cell::ptr_eq(receiver, root))
        {
            return Ok(());
        }
        Err(Halt::Trap(format!(
            "behavior exclusion `{spelling}` requires the current activation's original Build value"
        )))
    }

    fn record_executed_exclusion(
        &mut self,
        site: ExecutedBehaviorExclusionSite,
        kind: ExecutedBehaviorExclusionKind,
        frame: &Frame,
    ) -> EvalResult<()> {
        self.executed_behavior_exclusions
            .try_reserve(1)
            .map_err(|_| {
                Halt::Resource("behavior-exclusion result allocation was refused".to_owned())
            })?;
        self.executed_behavior_exclusions
            .push(ExecutedBehaviorExclusion {
                machine: frame.machine_symbol,
                site,
                kind,
            });
        Ok(())
    }
}
