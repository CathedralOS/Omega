//! Compiler-owned product-type-schema descriptions (`BuildProduct::schema`).
//!
//! The query is a designated compiler operation, not an ordinary call: it
//! resolves one exact product data declaration under the query occurrence's
//! lexical package scope -- its own package, or a `public` declaration inside a
//! package the occurrence's package holds an authorized product dependency on
//! -- and hands back an opaque `ProductTypeSchema` marker.
//! The semantic payload stays in the evaluator's private
//! `product_schema_descriptions` table, so evaluated code can carry the
//! marker but cannot read, convert, or fabricate the selection. No product
//! receiver, body, initializer, or provider executes -- the query is a
//! logical lookup over the authored frontier. `ProductTypeSchema::path(&schema)`
//! is the single inspection: it reads the description table and returns the
//! selected declaration's canonical path as ordinary text. Schema and provider
//! path inspection share the exact-operation dispatch below; their issued
//! description tables and role checks remain separate.

use super::product_providers::PRODUCT_PROVIDER_DESCRIPTION_TYPE;
use crate::interpreter::evaluator::{
    BTreeMap, EvalResult, Evaluator, ExpressionHandle, Frame, Halt, SymbolHandle, Value,
};

/// The marker type name carried by issued `ProductTypeSchema` values. An
/// authored `ProductTypeSchema {}` has the declared type name instead and is
/// refused by `described_product_schema` below.
const PRODUCT_SCHEMA_DESCRIPTION_TYPE: &str = "$OmegaBuildProductTypeSchema";

enum ProductDescriptionKind {
    Schema,
    Provider,
}

impl<'program> Evaluator<'program> {
    /// Inspect one issued description through its exact toolchain operation.
    /// Both call positions use this owner: discarding the result cannot skip
    /// marker/index validation, and unrelated `path` calls evaluate no operands
    /// here. The role is selected by declaration identity, never marker shape.
    pub(in crate::interpreter::evaluator) fn try_product_description_path_call(
        &mut self,
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
        frame: &mut Frame,
    ) -> EvalResult<Option<Value>> {
        if !target.is_valid() {
            return Ok(None);
        }
        let kind = if self.exact_build_facet_method("ProductTypeSchema", "path", target) {
            ProductDescriptionKind::Schema
        } else if self.exact_build_facet_method("ProductProviderRef", "path", target) {
            ProductDescriptionKind::Provider
        } else {
            return Ok(None);
        };
        let [description] = arguments else {
            return Err(Halt::Trap(
                "product-description path access requires one explicit description".to_owned(),
            ));
        };
        let description = self.eval_argument(*description, frame)?;
        let description = self.deref_cell(description);
        let (marker, rejection) = match kind {
            ProductDescriptionKind::Schema => (
                PRODUCT_SCHEMA_DESCRIPTION_TYPE,
                "product-schema path access requires a compiler-issued ProductTypeSchema",
            ),
            ProductDescriptionKind::Provider => (
                PRODUCT_PROVIDER_DESCRIPTION_TYPE,
                "product-provider path access requires a compiler-issued ProductProviderRef",
            ),
        };
        if !matches!(&*description.borrow(), Value::Struct { type_name, .. } if **type_name == *marker)
        {
            return Err(Halt::Trap(rejection.to_owned()));
        }
        let path = match kind {
            ProductDescriptionKind::Schema => {
                let index = self.described_product_schema(&description.borrow())?;
                self.product_schema_descriptions[index]
                    .canonical_path
                    .clone()
            }
            ProductDescriptionKind::Provider => {
                let index = self.described_product_provider(&description.borrow())?;
                self.product_provider_descriptions[index]
                    .canonical_path
                    .clone()
            }
        };
        Ok(Some(self.allocate_text(path.into_bytes())?))
    }

    /// Resolve an explicit facet query under the call's lexical package scope.
    pub(in crate::interpreter::evaluator) fn try_build_product_schema_call(
        &mut self,
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
        occurrence: source::SourceSpan,
        frame: &mut Frame,
    ) -> EvalResult<Option<Value>> {
        self.try_build_product_description_call(
            "schema",
            target,
            arguments,
            occurrence,
            frame,
            "product schema selection requires an explicit facet and path",
            |engine, path, frame| engine.eval_product_schema_operand(path, frame),
            |engine, path, occurrence| engine.issue_product_schema_description(path, occurrence),
        )
    }

    /// Rejoin a `ProductTypeSchema` runtime value to the description the
    /// compiler issued for it. Only the evaluator's own marker shape is
    /// admitted: an authored `ProductTypeSchema {}`, a foreign struct, or a
    /// description index outside the issued table cannot satisfy this check.
    /// `Build::bind` consumption goes through `described_product_entry`, whose
    /// distinct marker name makes a schema description unusable as an entry.
    pub(in crate::interpreter::evaluator) fn described_product_schema(
        &self,
        value: &Value,
    ) -> EvalResult<usize> {
        let Value::Struct {
            type_name, fields, ..
        } = value
        else {
            return Err(Halt::Trap(
                "product-schema inspection is not a compiler-issued product schema description"
                    .to_owned(),
            ));
        };
        if **type_name != *PRODUCT_SCHEMA_DESCRIPTION_TYPE {
            return Err(Halt::Trap(
                "product-schema inspection is not a compiler-issued product schema description"
                    .to_owned(),
            ));
        }
        let index = super::required_index(fields, "description", "product schema description")?;
        if index >= self.product_schema_descriptions.len() {
            return Err(Halt::Trap(
                "product schema description names an unknown description".to_owned(),
            ));
        }
        Ok(index)
    }

    fn issue_product_schema_description(
        &mut self,
        path: &[u8],
        occurrence: source::SourceSpan,
    ) -> EvalResult<Value> {
        let selected = validation::product_queries::describe_schema(
            self.product_declarations,
            occurrence,
            path,
        )
        .map_err(Halt::Trap)?;
        let index = self.product_schema_descriptions.len();
        self.product_schema_descriptions
            .try_reserve(1)
            .map_err(|_| {
                Halt::Resource("product-schema description allocation was refused".to_owned())
            })?;
        self.product_schema_descriptions.push(selected);
        let index = i64::try_from(index).map_err(|_| {
            Halt::Resource("product-schema description index overflowed".to_owned())
        })?;
        let description = self.allocate_cell(Value::Int(index))?;
        Ok(Value::Struct {
            type_symbol: SymbolHandle::invalid(),
            type_name: PRODUCT_SCHEMA_DESCRIPTION_TYPE.into(),
            fields: BTreeMap::from([("description".into(), description)]),
        })
    }

    fn eval_product_schema_operand(
        &mut self,
        expression: ExpressionHandle,
        frame: &mut Frame,
    ) -> EvalResult<Vec<u8>> {
        self.eval_byte_data_operand(
            expression,
            frame,
            "product schema operand",
            "product schema operands",
        )
    }
}
