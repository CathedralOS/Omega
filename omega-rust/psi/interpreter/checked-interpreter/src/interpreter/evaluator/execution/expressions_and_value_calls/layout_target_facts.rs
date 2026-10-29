//! Read-only observation of an evaluator-issued target context. The exact
//! sealed declaration selects this operation; its argument must independently
//! retain the private issuer tag. Authored records cannot supply that tag.
use crate::interpreter::evaluator::{EvalResult, Evaluator, Frame, Halt, Value, trap};

impl Evaluator<'_> {
    pub(super) fn try_layout_target_facts(
        &mut self,
        call: &typed_trees::expression::TableCallExpression,
        frame: &mut Frame,
    ) -> EvalResult<Option<Value>> {
        let Some(result) = self
            .program
            .layout_target_facts_result(call.target_symbol)
            .map_err(|reason| Halt::Trap(reason.into()))?
        else {
            return Ok(None);
        };
        let [argument] = self
            .program
            .expression_table
            .expression_handles(call.arguments)
        else {
            return trap("layout target snapshot requires its single shared context argument");
        };
        let value = self.eval_expression(*argument, frame)?;
        let value = match value {
            Value::Ref(reference) => self.read_reference(&reference)?,
            value => value,
        };
        let Value::Struct {
            type_name, fields, ..
        } = value
        else {
            return trap("layout target snapshot did not receive an issued context");
        };
        if type_name.as_ref() != "@compiler-layout-target"
            || fields.len() != 4
            || ![
                "profile",
                "pointer_size",
                "pointer_alignment",
                "little_endian",
            ]
            .iter()
            .all(|name| fields.contains_key(*name))
        {
            return trap("layout target snapshot requires the exact compiler-issued context");
        }
        // The ordinary result may be mutated by its owner. Its field homes
        // must therefore be fresh even before ordinary copy/arrival handling.
        // Immutable text backing may be shared; replacing the result's profile
        // descriptor still cannot change the issuer's private field home.
        let fields = fields
            .into_iter()
            .map(|(name, cell)| {
                let value = cell.borrow().clone();
                self.allocate_cell(value).map(|cell| (name, cell))
            })
            .collect::<EvalResult<_>>()?;
        Ok(Some(Value::Struct {
            type_symbol: result,
            type_name: self.program.symbols.name(result).into(),
            fields,
        }))
    }
}
