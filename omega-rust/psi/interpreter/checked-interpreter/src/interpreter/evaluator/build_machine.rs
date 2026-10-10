//! The build machine's facets: what an `omega build` evaluation reads, writes
//! and must account for, as opposed to ordinary program execution.
//!
//! Each module is one facet the build machine exposes to authored `build.omg`
//! code, and each contributes methods to `Evaluator` rather than a type of its
//! own: `paths` and `log` are the rooted path vocabulary and the build log,
//! `root_bindings` and `provider_selections` are what the build declares,
//! `product_entries`, `product_providers` and `product_schemas` are what it
//! produces, `output_obligations` is what it still owes at the end, and
//! `behavior_exclusions` is what it has been told not to do.

pub(in crate::interpreter::evaluator) mod behavior_exclusions;
pub(in crate::interpreter::evaluator) mod log;
pub(in crate::interpreter::evaluator) mod output_obligations;
pub(in crate::interpreter::evaluator) mod paths;
pub(in crate::interpreter::evaluator) mod product_declarations;
pub(in crate::interpreter::evaluator) mod product_entries;
pub(in crate::interpreter::evaluator) mod product_providers;
pub(in crate::interpreter::evaluator) mod product_schemas;
pub(in crate::interpreter::evaluator) mod provider_selections;
pub(in crate::interpreter::evaluator) mod root_bindings;

use super::{Cell, EvalResult, Evaluator, ExpressionHandle, Frame, Halt, Value};

impl Evaluator<'_> {
    /// A Build declaration borrows this activation's original cell. A value
    /// with the same type is not authority to configure the product.
    pub(in crate::interpreter::evaluator) fn require_root_build_argument(
        &self,
        builder: Cell,
        spelling: &str,
    ) -> EvalResult<()> {
        if self
            .root_build
            .as_ref()
            .is_some_and(|root| Cell::ptr_eq(&builder, root))
        {
            return Ok(());
        }
        Err(Halt::Trap(format!(
            "Build operation `{spelling}` requires the current activation's original Build value"
        )))
    }

    /// Byte-data operand decoding shared by the build-machine operand
    /// evaluators: a `Str`, or an integer array of bytes — anything else is a
    /// trap under the caller's operand names.
    pub(in crate::interpreter::evaluator) fn eval_byte_data_operand(
        &mut self,
        expression: ExpressionHandle,
        frame: &mut Frame,
        operand: &str,
        operands: &str,
    ) -> EvalResult<Vec<u8>> {
        match self.eval_expression(expression, frame)? {
            Value::Str(bytes) => Ok(bytes.borrow().to_vec()),
            Value::Array(cells) => cells
                .iter()
                .map(|cell| {
                    cell.borrow()
                        .as_int()
                        .and_then(|byte| u8::try_from(byte).ok())
                        .ok_or_else(|| Halt::Trap(format!("{operand} contains a non-byte element")))
                })
                .collect(),
            other => Err(Halt::Trap(format!(
                "{operands} must be byte data, got {other:?}"
            ))),
        }
    }
}

/// Reads a required integer index field off a compiler-issued struct value —
/// the extraction every build-machine producer performs before consulting
/// its own descriptor tables.
fn required_index(
    fields: &std::collections::BTreeMap<crate::value::ValueName, Cell>,
    key: &str,
    label: &str,
) -> Result<usize, Halt> {
    fields
        .get(key)
        .and_then(|cell| cell.borrow().as_int())
        .and_then(|index| usize::try_from(index).ok())
        .ok_or_else(|| Halt::Trap(format!("{label} carries no valid {key} index")))
}

/// Rejoin a `Product*Ref` marker to the description index the compiler issued
/// for it: marker struct shape, expected marker name, then the `description`
/// field's index.
fn described_product_index(
    value: &Value,
    expected_type: &str,
    guard_message: &'static str,
    index_label: &'static str,
) -> EvalResult<usize> {
    let Value::Struct {
        type_name, fields, ..
    } = value
    else {
        return Err(Halt::Trap(guard_message.to_owned()));
    };
    if &**type_name != expected_type {
        return Err(Halt::Trap(guard_message.to_owned()));
    }
    required_index(fields, "description", index_label)
}
