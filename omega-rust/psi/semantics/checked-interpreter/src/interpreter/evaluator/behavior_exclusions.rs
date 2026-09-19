//! Executed behavior exclusions: `builder.exclude_crash(cause)` and
//! `builder.exclude_service<Trait>()` recorded as they evaluate.
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
use super::{
    Cell, EvalResult, Evaluator, ExpressionHandle, Frame, Halt, SymbolHandle, TableCall, Value,
    trap,
};
use crate::{
    ExecutedBehaviorExclusion, ExecutedBehaviorExclusionKind, ExecutedBehaviorExclusionSite,
};
use typed_trees::statement::StatementHandle;

impl<'program> Evaluator<'program> {
    /// A `StatementNode::Call` that is one of the two exclusion spellings.
    /// `exclude_crash` arrives as an ordinary resolved call to the exact
    /// toolchain `Build::exclude_crash` machine; `exclude_service<Trait>` is
    /// the parser-carved marker (no resolved target machine). Neither body
    /// runs — the call IS the selection.
    pub(super) fn try_behavior_exclusion_statement(
        &mut self,
        statement: StatementHandle,
        call: &TableCall,
        frame: &Frame,
    ) -> EvalResult<Option<Value>> {
        // `exclude_service` is the parser-carved marker (no resolved target
        // machine); `exclude_crash` resolves to the exact toolchain
        // `Build::exclude_crash` state or arrives unresolved under the same
        // name. The name gates first: `exact_build_facet_method` admits an
        // unresolved target by name alone, so ordering by name keeps an
        // unrelated unresolved call out of the crash branch.
        let kind = if call.target.as_str() == "exclude_service" && !call.target_symbol.is_valid() {
            ExecutedBehaviorExclusionKind::Service
        } else if call.target.as_str() == "exclude_crash"
            && self.exact_build_facet_method("Build", "exclude_crash", call.target_symbol)
        {
            ExecutedBehaviorExclusionKind::CrashCause {
                case_symbol: self.evaluated_crash_cause(
                    self.program
                        .statement_table
                        .expression_handles(call.arguments),
                    frame,
                )?,
            }
        } else {
            return Ok(None);
        };
        self.require_root_build_receiver(
            self.statement_receiver_cell(call.receiver, frame)?,
            call.target.as_str(),
        )?;
        self.record_executed_exclusion(
            ExecutedBehaviorExclusionSite::Statement(statement),
            kind,
            frame,
        )?;
        Ok(Some(Value::Unit))
    }

    /// The expression-call twin of [`Self::try_behavior_exclusion_statement`]:
    /// `handle` is the `ExpressionNode::Call`'s own handle wherever it sat —
    /// statement expression or nested position — since an evaluated call is
    /// a selection regardless of the surrounding syntactic slot.
    pub(super) fn try_behavior_exclusion_value_call(
        &mut self,
        handle: ExpressionHandle,
        call: &typed_trees::expression::TableCallExpression,
        frame: &Frame,
    ) -> EvalResult<Option<Value>> {
        let kind = if call.target.as_str() == "exclude_service" && !call.target_symbol.is_valid() {
            ExecutedBehaviorExclusionKind::Service
        } else if call.target.as_str() == "exclude_crash"
            && self.exact_build_facet_method("Build", "exclude_crash", call.target_symbol)
        {
            ExecutedBehaviorExclusionKind::CrashCause {
                case_symbol: self.evaluated_crash_cause(
                    self.program
                        .expression_table
                        .expression_handles(call.arguments),
                    frame,
                )?,
            }
        } else {
            return Ok(None);
        };
        let receiver = if call.receiver.is_valid() {
            match self.resolve_place(call.receiver, frame) {
                Ok(cell) => Some(self.deref_cell(cell)),
                Err(_) => None,
            }
        } else {
            None
        };
        self.require_root_build_receiver(receiver, call.target.as_str())?;
        self.record_executed_exclusion(
            ExecutedBehaviorExclusionSite::Expression(handle),
            kind,
            frame,
        )?;
        Ok(Some(Value::Unit))
    }

    /// Evaluate the `exclude_crash` argument and recover the exact toolchain
    /// `CrashCause` variant the value carries. The argument is an ordinary
    /// evaluated expression — a bound local or a computed case selects as
    /// much as a literal `CrashCause::Trap` spelling.
    fn evaluated_crash_cause(
        &mut self,
        arguments: &[ExpressionHandle],
        frame: &Frame,
    ) -> EvalResult<SymbolHandle> {
        let [argument] = arguments else {
            return trap("behavior exclusion `exclude_crash` takes exactly one CrashCause value");
        };
        let Value::Enum {
            type_symbol,
            variant_name,
            payload,
        } = self.eval_expression(*argument, frame)?
        else {
            return trap(
                "behavior exclusion `exclude_crash` argument did not evaluate to a compiler-owned CrashCause case",
            );
        };
        let is_toolchain_cause = type_symbol != SymbolHandle::invalid()
            && self.symbol_has_build_prelude_source(type_symbol)
            && self.program.data_definitions().iter().any(|definition| {
                definition.symbol == type_symbol && definition.name.as_str() == "CrashCause"
            });
        if !is_toolchain_cause {
            return trap(
                "behavior exclusion `exclude_crash` argument did not evaluate to a compiler-owned CrashCause case",
            );
        }
        if !payload.is_empty() {
            return trap(
                "compiler-owned CrashCause case unexpectedly carries a payload".to_owned(),
            );
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
                            if variant.name.as_str() == variant_name =>
                        {
                            Some(variant.symbol)
                        }
                        _ => None,
                    })
            });
        variant.ok_or_else(|| {
            Halt::Trap(
                "evaluated CrashCause value names no declared compiler-owned case".to_owned(),
            )
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
