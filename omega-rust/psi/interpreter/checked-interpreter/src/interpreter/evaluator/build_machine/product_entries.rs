//! Compiler-owned product-entry descriptions (`BuildProduct::entry`).
//!
//! The query is a designated compiler operation, not an ordinary call: it
//! resolves one exact product machine under the query occurrence's lexical
//! package scope -- its own package, or a `public` declaration inside a package
//! the occurrence's package holds an authorized product dependency on -- and
//! hands back an opaque `ProductEntryRef` marker. The
//! semantic payload stays in the evaluator's private
//! `product_entry_descriptions` table, so evaluated code can carry the marker
//! but cannot read, convert, or fabricate the selection. No product receiver,
//! body, initializer, or provider executes -- the query is a logical lookup
//! over the authored frontier.

use crate::interpreter::evaluator::{
    BTreeMap, EvalResult, Evaluator, ExpressionHandle, Frame, Halt, SymbolHandle, Value,
};
use language_semantics::declaration_selection::DescribedProductEntry;
/// The compiler-created `Build.product` facet value inside the canonical
/// Build activation. An authored `BuildProduct {}` has the same static shape
/// but never this marker name, so it cannot perform the query.
pub(in crate::interpreter::evaluator) const PRODUCT_FACET_TYPE: &str = "$OmegaBuildProductFacet";

/// The marker type name carried by issued `ProductEntryRef` values. An
/// authored `ProductEntryRef {}` has the declared type name instead and is
/// refused by `described_product_entry` below.
const PRODUCT_ENTRY_DESCRIPTION_TYPE: &str = "$OmegaBuildProductEntry";

impl<'program> Evaluator<'program> {
    /// Execute the exact compiler operation with its explicit activation facet.
    /// Declaration namespaces never provide runtime selection authority.
    pub(in crate::interpreter::evaluator) fn try_build_product_entry_call(
        &mut self,
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
        occurrence: source::SourceSpan,
        frame: &mut Frame,
    ) -> EvalResult<Option<Value>> {
        if !target.is_valid() || !self.exact_build_facet_method("BuildProduct", "entry", target) {
            return Ok(None);
        }
        let [facet, path, slot] = arguments else {
            return Err(Halt::Trap(
                "product entry selection requires an explicit facet and path and slot".to_owned(),
            ));
        };
        self.require_product_facet(*facet, frame)?;
        let path = self.eval_product_entry_operand(*path, frame)?;
        let slot = self.eval_product_entry_operand(*slot, frame)?;
        self.issue_product_entry_description(&path, &slot, occurrence)
            .map(Some)
    }

    /// Validate the ordinary explicit facet operand shared by Product queries.
    pub(in crate::interpreter::evaluator) fn require_product_facet(
        &mut self,
        expression: ExpressionHandle,
        frame: &mut Frame,
    ) -> EvalResult<()> {
        let facet = self.eval_argument(expression, frame)?;
        let facet = self.deref_cell(facet);
        if !matches!(&*facet.borrow(), Value::Struct { type_name, .. } if **type_name == *PRODUCT_FACET_TYPE)
        {
            return Err(Halt::Trap(
                "product selection requires the compiler-issued Build.product facet".to_owned(),
            ));
        }
        Ok(())
    }

    /// Rejoin a `ProductEntryRef` runtime value to the description the
    /// compiler issued for it. Only the evaluator's own marker shape is
    /// admitted: an authored `ProductEntryRef {}`, a foreign struct, or a
    /// description index outside the issued table cannot satisfy this check.
    pub(in crate::interpreter::evaluator) fn described_product_entry(
        &self,
        value: &Value,
    ) -> EvalResult<DescribedProductEntry> {
        let index = super::described_product_index(
            value,
            PRODUCT_ENTRY_DESCRIPTION_TYPE,
            "root binding operand is not a compiler-issued product entry description",
            "product entry description",
        )?;
        self.product_entry_descriptions
            .get(index)
            .cloned()
            .ok_or_else(|| {
                Halt::Trap("product entry description names an unknown description".to_owned())
            })
    }

    fn issue_product_entry_description(
        &mut self,
        path: &[u8],
        slot: &[u8],
        occurrence: source::SourceSpan,
    ) -> EvalResult<Value> {
        let compatibility = self.product_entry_compatibility.ok_or_else(|| {
            Halt::Trap(
                "product entry selection requires the build owner's slot compatibility check"
                    .to_owned(),
            )
        })?;
        let selected = validation::product_queries::describe_entry(
            self.product_declarations,
            occurrence,
            path,
            slot,
            compatibility,
        )
        .map_err(Halt::Trap)?;
        let index = self.product_entry_descriptions.len();
        self.product_entry_descriptions
            .try_reserve(1)
            .map_err(|_| {
                Halt::Resource("product-entry description allocation was refused".to_owned())
            })?;
        self.product_entry_descriptions.push(selected);
        let index = i64::try_from(index)
            .map_err(|_| Halt::Resource("product-entry description index overflowed".to_owned()))?;
        let description = self.allocate_cell(Value::Int(index))?;
        Ok(Value::Struct {
            type_symbol: SymbolHandle::invalid(),
            type_name: PRODUCT_ENTRY_DESCRIPTION_TYPE.into(),
            fields: BTreeMap::from([("description".into(), description)]),
        })
    }

    fn eval_product_entry_operand(
        &mut self,
        expression: ExpressionHandle,
        frame: &mut Frame,
    ) -> EvalResult<Vec<u8>> {
        self.eval_byte_data_operand(
            expression,
            frame,
            "product entry operand",
            "product entry operands",
        )
    }
}
