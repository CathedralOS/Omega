//! Compiler-owned product-provider descriptions (`BuildProduct::provider`).
//!
//! The query is a designated compiler operation, not an ordinary call: it
//! resolves one exact product provider declaration under the query
//! occurrence's lexical package scope -- its own package, or a `public`
//! declaration inside a package the occurrence's package holds an authorized
//! product dependency on -- and hands back an opaque
//! `ProductProviderRef` marker. A provider declaration is a named
//! implementation, so plain data, machines,
//! and other declarations cannot mint this description kind. The semantic
//! payload stays in the evaluator's private `product_provider_descriptions`
//! table, so evaluated code can carry the marker but cannot read, convert, or
//! fabricate the selection. No product receiver, body, initializer, or
//! provider executes -- the query is a logical lookup over the authored
//! frontier. `ProductProviderRef::path(&provider)` is the single sanctioned
//! inspection: the shared operation in `product_schemas` reads this role's
//! description table and returns the declaration's canonical path as text.

use crate::interpreter::evaluator::{
    BTreeMap, EvalResult, Evaluator, ExpressionHandle, Frame, Halt, SymbolHandle, Value,
};

/// The marker type name carried by issued `ProductProviderRef` values. An
/// authored `ProductProviderRef {}` has the declared type name instead and is
/// refused by `described_product_provider` below.
pub(super) const PRODUCT_PROVIDER_DESCRIPTION_TYPE: &str = "$OmegaBuildProductProvider";

impl<'program> Evaluator<'program> {
    /// Resolve an explicit facet query under the call's lexical package scope.
    pub(in crate::interpreter::evaluator) fn try_build_product_provider_call(
        &mut self,
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
        occurrence: source::SourceSpan,
        frame: &mut Frame,
    ) -> EvalResult<Option<Value>> {
        self.try_build_product_description_call(
            "provider",
            target,
            arguments,
            occurrence,
            frame,
            "product provider selection requires an explicit facet and path",
            |engine, path, frame| engine.eval_product_provider_operand(path, frame),
            |engine, path, occurrence| engine.issue_product_provider_description(path, occurrence),
        )
    }

    /// Rejoin a `ProductProviderRef` runtime value to the description the
    /// compiler issued for it. Only the evaluator's own marker shape is
    /// admitted: an authored `ProductProviderRef {}`, a foreign struct, or a
    /// description index outside the issued table cannot satisfy this check.
    /// The entry and schema kinds stay distinct behind their own marker
    /// names, so a provider description cannot stand in for either.
    pub(in crate::interpreter::evaluator) fn described_product_provider(
        &self,
        value: &Value,
    ) -> EvalResult<usize> {
        let index = super::described_product_index(
            value,
            PRODUCT_PROVIDER_DESCRIPTION_TYPE,
            "product-provider inspection is not a compiler-issued product provider description",
            "product provider description",
        )?;
        if index >= self.product_provider_descriptions.len() {
            return Err(Halt::Trap(
                "product provider description names an unknown description".to_owned(),
            ));
        }
        Ok(index)
    }

    fn issue_product_provider_description(
        &mut self,
        path: &[u8],
        occurrence: source::SourceSpan,
    ) -> EvalResult<Value> {
        let selected = validation::product_queries::describe_provider(
            self.product_declarations,
            occurrence,
            path,
        )
        .map_err(Halt::Trap)?;
        let index = self.product_provider_descriptions.len();
        self.product_provider_descriptions
            .try_reserve(1)
            .map_err(|_| {
                Halt::Resource("product-provider description allocation was refused".to_owned())
            })?;
        self.product_provider_descriptions.push(selected);
        let index = i64::try_from(index).map_err(|_| {
            Halt::Resource("product-provider description index overflowed".to_owned())
        })?;
        let description = self.allocate_cell(Value::Int(index))?;
        Ok(Value::Struct {
            type_symbol: SymbolHandle::invalid(),
            type_name: PRODUCT_PROVIDER_DESCRIPTION_TYPE.into(),
            fields: BTreeMap::from([("description".into(), description)]),
        })
    }

    fn eval_product_provider_operand(
        &mut self,
        expression: ExpressionHandle,
        frame: &mut Frame,
    ) -> EvalResult<Vec<u8>> {
        self.eval_byte_data_operand(
            expression,
            frame,
            "product provider operand",
            "product provider operands",
        )
    }
}
