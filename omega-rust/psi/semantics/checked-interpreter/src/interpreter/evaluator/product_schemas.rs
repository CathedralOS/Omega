//! Compiler-owned product-type-schema descriptions (`Build.product.schema`).
//!
//! The query is a designated compiler operation, not an ordinary call: it
//! resolves one exact product data declaration under the query occurrence's
//! lexical package scope -- its own package, or a `pub` declaration inside a
//! package the occurrence's package holds an authorized product dependency on
//! -- and hands back an opaque `ProductTypeSchema` marker.
//! The semantic payload stays in the evaluator's private
//! `product_schema_descriptions` table, so evaluated code can carry the
//! marker but cannot read, convert, or fabricate the selection. No product
//! receiver, body, initializer, or provider executes -- the query is a
//! logical lookup over the authored frontier. `schema.path()` is the single
//! inspection: it reads the description table and returns the selected
//! declaration's canonical path as ordinary text.

use super::product_entries::PRODUCT_FACET_TYPE;
use super::{BTreeMap, EvalResult, Evaluator, ExpressionHandle, Frame, Halt, SymbolHandle, Value};

/// The marker type name carried by issued `ProductTypeSchema` values. An
/// authored `ProductTypeSchema {}` has the declared type name instead and is
/// refused by `described_product_schema` below.
const PRODUCT_SCHEMA_DESCRIPTION_TYPE: &str = "$OmegaBuildProductTypeSchema";

impl<'program> Evaluator<'program> {
    /// `builder.product.schema(path)` as a value-position call: resolve
    /// `path` under the CALL's lexical package scope and return an opaque
    /// description marker.
    pub(super) fn try_build_product_schema_value_call(
        &mut self,
        handle: ExpressionHandle,
        call: &typed_trees::expression::TableCallExpression,
        frame: &Frame,
    ) -> EvalResult<Option<Value>> {
        if call.target.as_str() != "schema" || !call.receiver.is_valid() {
            return Ok(None);
        }
        let receiver = self.resolve_place(call.receiver, frame)?;
        let receiver = self.deref_cell(receiver);
        let is_facet = matches!(
            &*receiver.borrow(),
            Value::Struct { type_name, .. } if type_name == PRODUCT_FACET_TYPE
        );
        if !is_facet {
            // An authored `BuildProduct` value can select the toolchain
            // machine through member resolution, but only the compiler-issued
            // facet may carry selection authority into a description.
            if call.target_symbol.is_valid()
                && self.exact_build_facet_method("BuildProduct", "schema", call.target_symbol)
            {
                return Err(Halt::Trap(
                    "product schema selection requires the compiler-issued Build.product facet"
                        .to_owned(),
                ));
            }
            return Ok(None);
        }
        if !self.exact_build_facet_method("BuildProduct", "schema", call.target_symbol) {
            return Err(Halt::Trap(
                "product schema selection did not select the exact toolchain machine".to_owned(),
            ));
        }
        let occurrence = self.program.expression_table.source_span(handle);
        let arguments = self
            .program
            .expression_table
            .expression_handles(call.arguments);
        let [path] = arguments else {
            return Err(Halt::Trap(
                "product schema selection requires a path".to_owned(),
            ));
        };
        let path = self.eval_product_schema_operand(*path, frame)?;
        self.issue_product_schema_description(&path, occurrence)
            .map(Some)
    }

    /// The statement-position twin: `builder.product.schema(path);` still
    /// performs and checks the query, then drops the description.
    pub(super) fn try_build_product_schema_statement(
        &mut self,
        call: &typed_trees::statement::TableCall,
        frame: &Frame,
    ) -> EvalResult<bool> {
        if call.target.as_str() != "schema" || call.receiver.is_empty() {
            return Ok(false);
        }
        let Some(receiver) = self.statement_receiver_cell(call.receiver, frame)? else {
            return Ok(false);
        };
        let is_facet = matches!(
            &*receiver.borrow(),
            Value::Struct { type_name, .. } if type_name == PRODUCT_FACET_TYPE
        );
        if !is_facet {
            if call.target_symbol.is_valid()
                && self.exact_build_facet_method("BuildProduct", "schema", call.target_symbol)
            {
                return Err(Halt::Trap(
                    "product schema selection requires the compiler-issued Build.product facet"
                        .to_owned(),
                ));
            }
            return Ok(false);
        }
        if !self.exact_build_facet_method("BuildProduct", "schema", call.target_symbol) {
            return Err(Halt::Trap(
                "product schema selection did not select the exact toolchain machine".to_owned(),
            ));
        }
        let arguments = self
            .program
            .statement_table
            .expression_handles(call.arguments);
        let [path] = arguments else {
            return Err(Halt::Trap(
                "product schema selection requires a path".to_owned(),
            ));
        };
        let path = self.eval_product_schema_operand(*path, frame)?;
        self.issue_product_schema_description(&path, call.source_span)?;
        Ok(true)
    }

    /// `schema.path()` as a value-position call: inspect the description and
    /// return the selected declaration's canonical path as text. The marker
    /// check is identical to `RequiredOutput::path`: an authored
    /// `ProductTypeSchema {}` that still resolves the toolchain machine is
    /// refused rather than allowed to mimic inspection.
    pub(super) fn try_product_type_schema_path_value_call(
        &mut self,
        call: &typed_trees::expression::TableCallExpression,
        frame: &Frame,
    ) -> EvalResult<Option<Value>> {
        if call.target.as_str() != "path" || !call.receiver.is_valid() {
            return Ok(None);
        }
        let receiver = self.resolve_place(call.receiver, frame)?;
        let receiver = self.deref_cell(receiver);
        let is_marker = matches!(
            &*receiver.borrow(),
            Value::Struct { type_name, .. } if type_name == PRODUCT_SCHEMA_DESCRIPTION_TYPE
        );
        if !is_marker {
            if call.target_symbol.is_valid()
                && self.exact_build_facet_method("ProductTypeSchema", "path", call.target_symbol)
            {
                return Err(Halt::Trap(
                    "product-schema path access requires a compiler-issued ProductTypeSchema"
                        .to_owned(),
                ));
            }
            return Ok(None);
        }
        if !self.exact_build_facet_method("ProductTypeSchema", "path", call.target_symbol) {
            return Err(Halt::Trap(
                "product-schema path access did not select the exact toolchain machine".to_owned(),
            ));
        }
        let index = self.described_product_schema(&receiver.borrow())?;
        let path = self.product_schema_descriptions[index]
            .canonical_path
            .clone()
            .into_bytes();
        Ok(Some(self.allocate_text(path)?))
    }

    /// The statement-position twin of `schema.path()`; the declared path is
    /// evaluated and dropped.
    pub(super) fn try_product_type_schema_path_statement(
        &mut self,
        call: &typed_trees::statement::TableCall,
        frame: &Frame,
    ) -> EvalResult<bool> {
        if call.target.as_str() != "path" || call.receiver.is_empty() {
            return Ok(false);
        }
        let Some(receiver) = self.statement_receiver_cell(call.receiver, frame)? else {
            return Ok(false);
        };
        let is_marker = matches!(
            &*receiver.borrow(),
            Value::Struct { type_name, .. } if type_name == PRODUCT_SCHEMA_DESCRIPTION_TYPE
        );
        if !is_marker {
            if call.target_symbol.is_valid()
                && self.exact_build_facet_method("ProductTypeSchema", "path", call.target_symbol)
            {
                return Err(Halt::Trap(
                    "product-schema path access requires a compiler-issued ProductTypeSchema"
                        .to_owned(),
                ));
            }
            return Ok(false);
        }
        if !self.exact_build_facet_method("ProductTypeSchema", "path", call.target_symbol) {
            return Err(Halt::Trap(
                "product-schema path access did not select the exact toolchain machine".to_owned(),
            ));
        }
        self.described_product_schema(&receiver.borrow())?;
        Ok(true)
    }

    /// Rejoin a `ProductTypeSchema` runtime value to the description the
    /// compiler issued for it. Only the evaluator's own marker shape is
    /// admitted: an authored `ProductTypeSchema {}`, a foreign struct, or a
    /// description index outside the issued table cannot satisfy this check.
    /// `roots.bind` consumption goes through `described_product_entry`, whose
    /// distinct marker name makes a schema description unusable as an entry.
    pub(super) fn described_product_schema(&self, value: &Value) -> EvalResult<usize> {
        let Value::Struct {
            type_name, fields, ..
        } = value
        else {
            return Err(Halt::Trap(
                "product-schema inspection is not a compiler-issued product schema description"
                    .to_owned(),
            ));
        };
        if type_name != PRODUCT_SCHEMA_DESCRIPTION_TYPE {
            return Err(Halt::Trap(
                "product-schema inspection is not a compiler-issued product schema description"
                    .to_owned(),
            ));
        }
        let index = fields
            .get("description")
            .and_then(|cell| cell.borrow().as_int())
            .and_then(|index| usize::try_from(index).ok())
            .ok_or_else(|| {
                Halt::Trap(
                    "product schema description carries no valid description index".to_owned(),
                )
            })?;
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
        let name = std::str::from_utf8(path)
            .map_err(|_| Halt::Trap("product schema path must be canonical UTF-8".to_owned()))?;
        if name.is_empty() {
            return Err(Halt::Trap(
                "product schema selection requires a non-empty path".to_owned(),
            ));
        }
        // Strings are locators, not authority: `path` resolves under the
        // query occurrence's own authorized scope. A qualified `alias::rest`
        // spelling resolves `alias` through the occurrence package's
        // retained product dependencies and selects a `pub` declaration in
        // that exact target package -- a helper borrowing Build keeps its
        // operational capability but cannot enumerate the caller's product
        // namespace, and a build-scope alias authorizes nothing. A bare leaf
        // keeps the same-package declaration scan; a first segment that is
        // no product alias leaves only the occurrence's own package path
        // reading of the full spelling.
        let qualified = name.split_once("::");
        let authorized = qualified.and_then(|(alias, _)| {
            self.program
                .symbols
                .product_dependency_target(occurrence, alias)
        });
        let candidates = match (qualified, authorized) {
            (Some((_, rest)), Some(target)) => self
                .program
                .data_definitions()
                .iter()
                .filter(|definition| {
                    definition.is_public
                        && (definition.name.as_str() == rest
                            || self
                                .program
                                .symbols
                                .display_path(definition.symbol, "::")
                                .as_str()
                                == rest)
                        && self
                            .program
                            .symbols
                            .symbol_package_identity(definition.symbol)
                            == Some(target)
                })
                .collect::<Vec<_>>(),
            _ => self
                .program
                .data_definitions()
                .iter()
                .filter(|definition| {
                    (match qualified {
                        Some(_) => {
                            self.program
                                .symbols
                                .display_path(definition.symbol, "::")
                                .as_str()
                                == name
                        }
                        None => definition.name.as_str() == name,
                    }) && self
                        .program
                        .symbols
                        .symbol_source_span(definition.symbol)
                        .is_some_and(|span| {
                            self.program
                                .symbols
                                .same_product_package_instance(occurrence, span)
                        })
                })
                .collect::<Vec<_>>(),
        };
        let [definition] = candidates.as_slice() else {
            if !candidates.is_empty() {
                return Err(Halt::Trap(format!(
                    "product schema `{name}` is ambiguous within its package"
                )));
            }
            let probe = qualified.map_or(name, |(_, rest)| rest);
            let message = if self.program.data_definitions().iter().any(|definition| {
                definition.name.as_str() == probe
                    || self
                        .program
                        .symbols
                        .display_path(definition.symbol, "::")
                        .as_str()
                        == probe
            }) {
                format!(
                    "product schema `{name}` is not a product declaration visible from this build occurrence's package"
                )
            } else {
                format!(
                    "product schema `{name}` names no data declaration in this build occurrence's package"
                )
            };
            return Err(Halt::Trap(message));
        };
        let canonical_path = self.program.symbols.display_path(definition.symbol, "::");
        let index = self.product_schema_descriptions.len();
        self.product_schema_descriptions
            .try_reserve(1)
            .map_err(|_| {
                Halt::Resource("product-schema description allocation was refused".to_owned())
            })?;
        self.product_schema_descriptions
            .push(crate::DescribedProductSchema {
                schema_symbol: definition.symbol,
                canonical_path,
            });
        let index = i64::try_from(index).map_err(|_| {
            Halt::Resource("product-schema description index overflowed".to_owned())
        })?;
        let description = self.allocate_cell(Value::Int(index))?;
        Ok(Value::Struct {
            type_symbol: SymbolHandle::invalid(),
            type_name: PRODUCT_SCHEMA_DESCRIPTION_TYPE.to_owned(),
            fields: BTreeMap::from([("description".to_owned(), description)]),
        })
    }

    fn eval_product_schema_operand(
        &mut self,
        expression: ExpressionHandle,
        frame: &Frame,
    ) -> EvalResult<Vec<u8>> {
        match self.eval_expression(expression, frame)? {
            Value::Str(bytes) => Ok(bytes.borrow().to_vec()),
            Value::Array(cells) => cells
                .iter()
                .map(|cell| {
                    cell.borrow()
                        .as_int()
                        .and_then(|byte| u8::try_from(byte).ok())
                        .ok_or_else(|| {
                            Halt::Trap(
                                "product schema operand contains a non-byte element".to_owned(),
                            )
                        })
                })
                .collect(),
            other => Err(Halt::Trap(format!(
                "product schema operands must be byte data, got {other:?}"
            ))),
        }
    }
}
