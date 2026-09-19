//! Compiler-owned product-provider descriptions (`Build.product.provider`).
//!
//! The query is a designated compiler operation, not an ordinary call: it
//! resolves one exact product provider declaration under the query
//! occurrence's lexical package scope -- its own package, or a `pub`
//! declaration inside a package the occurrence's package holds an authorized
//! product dependency on -- and hands back an opaque
//! `ProductProviderRef` marker. A provider declaration is the nominal data
//! type owning at least one `satisfies` machine, so plain data, machines,
//! and other declarations cannot mint this description kind. The semantic
//! payload stays in the evaluator's private `product_provider_descriptions`
//! table, so evaluated code can carry the marker but cannot read, convert, or
//! fabricate the selection. No product receiver, body, initializer, or
//! provider executes -- the query is a logical lookup over the authored
//! frontier. `provider.path()` is the single sanctioned inspection: it reads
//! the description table and returns the selected declaration's canonical
//! path as ordinary text.

use super::product_entries::PRODUCT_FACET_TYPE;
use super::{BTreeMap, EvalResult, Evaluator, ExpressionHandle, Frame, Halt, SymbolHandle, Value};

/// The marker type name carried by issued `ProductProviderRef` values. An
/// authored `ProductProviderRef {}` has the declared type name instead and is
/// refused by `described_product_provider` below.
const PRODUCT_PROVIDER_DESCRIPTION_TYPE: &str = "$OmegaBuildProductProvider";

impl<'program> Evaluator<'program> {
    /// `builder.product.provider(path)` as a value-position call: resolve
    /// `path` under the CALL's lexical package scope and return an opaque
    /// description marker.
    pub(super) fn try_build_product_provider_value_call(
        &mut self,
        handle: ExpressionHandle,
        call: &typed_trees::expression::TableCallExpression,
        frame: &Frame,
    ) -> EvalResult<Option<Value>> {
        if call.target.as_str() != "provider" || !call.receiver.is_valid() {
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
                && self.exact_build_facet_method("BuildProduct", "provider", call.target_symbol)
            {
                return Err(Halt::Trap(
                    "product provider selection requires the compiler-issued Build.product facet"
                        .to_owned(),
                ));
            }
            return Ok(None);
        }
        if !self.exact_build_facet_method("BuildProduct", "provider", call.target_symbol) {
            return Err(Halt::Trap(
                "product provider selection did not select the exact toolchain machine".to_owned(),
            ));
        }
        let occurrence = self.program.expression_table.source_span(handle);
        let arguments = self
            .program
            .expression_table
            .expression_handles(call.arguments);
        let [path] = arguments else {
            return Err(Halt::Trap(
                "product provider selection requires a path".to_owned(),
            ));
        };
        let path = self.eval_product_provider_operand(*path, frame)?;
        self.issue_product_provider_description(&path, occurrence)
            .map(Some)
    }

    /// The statement-position twin: `builder.product.provider(path);` still
    /// performs and checks the query, then drops the description.
    pub(super) fn try_build_product_provider_statement(
        &mut self,
        call: &typed_trees::statement::TableCall,
        frame: &Frame,
    ) -> EvalResult<bool> {
        if call.target.as_str() != "provider" || call.receiver.is_empty() {
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
                && self.exact_build_facet_method("BuildProduct", "provider", call.target_symbol)
            {
                return Err(Halt::Trap(
                    "product provider selection requires the compiler-issued Build.product facet"
                        .to_owned(),
                ));
            }
            return Ok(false);
        }
        if !self.exact_build_facet_method("BuildProduct", "provider", call.target_symbol) {
            return Err(Halt::Trap(
                "product provider selection did not select the exact toolchain machine".to_owned(),
            ));
        }
        let arguments = self
            .program
            .statement_table
            .expression_handles(call.arguments);
        let [path] = arguments else {
            return Err(Halt::Trap(
                "product provider selection requires a path".to_owned(),
            ));
        };
        let path = self.eval_product_provider_operand(*path, frame)?;
        self.issue_product_provider_description(&path, call.source_span)?;
        Ok(true)
    }

    /// `provider.path()` as a value-position call: inspect the description
    /// and return the selected declaration's canonical path as text. The
    /// marker check is identical to `ProductTypeSchema::path`: an authored
    /// `ProductProviderRef {}` that still resolves the toolchain machine is
    /// refused rather than allowed to mimic inspection.
    pub(super) fn try_product_provider_ref_path_value_call(
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
            Value::Struct { type_name, .. } if type_name == PRODUCT_PROVIDER_DESCRIPTION_TYPE
        );
        if !is_marker {
            if call.target_symbol.is_valid()
                && self.exact_build_facet_method("ProductProviderRef", "path", call.target_symbol)
            {
                return Err(Halt::Trap(
                    "product-provider path access requires a compiler-issued ProductProviderRef"
                        .to_owned(),
                ));
            }
            return Ok(None);
        }
        if !self.exact_build_facet_method("ProductProviderRef", "path", call.target_symbol) {
            return Err(Halt::Trap(
                "product-provider path access did not select the exact toolchain machine"
                    .to_owned(),
            ));
        }
        let index = self.described_product_provider(&receiver.borrow())?;
        let path = self.product_provider_descriptions[index]
            .canonical_path
            .clone()
            .into_bytes();
        Ok(Some(self.allocate_text(path)?))
    }

    /// The statement-position twin of `provider.path()`; the declared path is
    /// evaluated and dropped.
    pub(super) fn try_product_provider_ref_path_statement(
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
            Value::Struct { type_name, .. } if type_name == PRODUCT_PROVIDER_DESCRIPTION_TYPE
        );
        if !is_marker {
            if call.target_symbol.is_valid()
                && self.exact_build_facet_method("ProductProviderRef", "path", call.target_symbol)
            {
                return Err(Halt::Trap(
                    "product-provider path access requires a compiler-issued ProductProviderRef"
                        .to_owned(),
                ));
            }
            return Ok(false);
        }
        if !self.exact_build_facet_method("ProductProviderRef", "path", call.target_symbol) {
            return Err(Halt::Trap(
                "product-provider path access did not select the exact toolchain machine"
                    .to_owned(),
            ));
        }
        self.described_product_provider(&receiver.borrow())?;
        Ok(true)
    }

    /// Rejoin a `ProductProviderRef` runtime value to the description the
    /// compiler issued for it. Only the evaluator's own marker shape is
    /// admitted: an authored `ProductProviderRef {}`, a foreign struct, or a
    /// description index outside the issued table cannot satisfy this check.
    /// The entry and schema kinds stay distinct behind their own marker
    /// names, so a provider description cannot stand in for either.
    pub(super) fn described_product_provider(&self, value: &Value) -> EvalResult<usize> {
        let Value::Struct {
            type_name, fields, ..
        } = value
        else {
            return Err(Halt::Trap(
                "product-provider inspection is not a compiler-issued product provider description"
                    .to_owned(),
            ));
        };
        if type_name != PRODUCT_PROVIDER_DESCRIPTION_TYPE {
            return Err(Halt::Trap(
                "product-provider inspection is not a compiler-issued product provider description"
                    .to_owned(),
            ));
        }
        let index = fields
            .get("description")
            .and_then(|cell| cell.borrow().as_int())
            .and_then(|index| usize::try_from(index).ok())
            .ok_or_else(|| {
                Halt::Trap(
                    "product provider description carries no valid description index".to_owned(),
                )
            })?;
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
        let name = std::str::from_utf8(path)
            .map_err(|_| Halt::Trap("product provider path must be canonical UTF-8".to_owned()))?;
        if name.is_empty() {
            return Err(Halt::Trap(
                "product provider selection requires a non-empty path".to_owned(),
            ));
        }
        // Strings are locators, not authority: `path` resolves under the
        // query occurrence's own authorized scope. A provider declaration is
        // the nominal data type owning at least one `satisfies` machine, so
        // plain data and non-data names never qualify. A qualified
        // `alias::rest` spelling resolves `alias` through the occurrence
        // package's retained product dependencies and selects a `pub`
        // declaration in that exact target package's product checked instance.
        // A helper borrowing Build keeps its operational capability, not the
        // caller's product namespace, and a build-scope alias authorizes nothing. A
        // bare leaf keeps the same-package scan; a first segment that is no
        // product alias leaves only the occurrence's own package path
        // reading of the full spelling.
        let is_provider = |symbol: SymbolHandle| {
            self.program.machines().iter().any(|machine| {
                machine.attached_data_symbol == symbol && !machine.satisfies.is_empty()
            })
        };
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
                        && is_provider(definition.symbol)
                        // The dependency alias qualifies a complete path;
                        // a different module's relative name is not a match.
                        && self.program.symbols.display_path(definition.symbol, "::") == rest
                        && self
                            .program
                            .symbols
                            .symbol_product_package_identity(definition.symbol)
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
                    }) && is_provider(definition.symbol)
                        && self
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
                    "product provider `{name}` is ambiguous within its package"
                )));
            }
            let probe = qualified.map_or(name, |(_, rest)| rest);
            let names_a_provider_elsewhere =
                self.program.data_definitions().iter().any(|definition| {
                    is_provider(definition.symbol)
                        && (definition.name.as_str() == probe
                            || self
                                .program
                                .symbols
                                .display_path(definition.symbol, "::")
                                .as_str()
                                == probe)
                });
            let message = if names_a_provider_elsewhere {
                format!(
                    "product provider `{name}` is not a provider declaration visible from this build occurrence's package"
                )
            } else {
                format!(
                    "product provider `{name}` names no provider declaration in this build occurrence's package"
                )
            };
            return Err(Halt::Trap(message));
        };
        let canonical_path = self.program.symbols.display_path(definition.symbol, "::");
        let index = self.product_provider_descriptions.len();
        self.product_provider_descriptions
            .try_reserve(1)
            .map_err(|_| {
                Halt::Resource("product-provider description allocation was refused".to_owned())
            })?;
        self.product_provider_descriptions
            .push(crate::DescribedProductProvider {
                provider_symbol: definition.symbol,
                canonical_path,
            });
        let index = i64::try_from(index).map_err(|_| {
            Halt::Resource("product-provider description index overflowed".to_owned())
        })?;
        let description = self.allocate_cell(Value::Int(index))?;
        Ok(Value::Struct {
            type_symbol: SymbolHandle::invalid(),
            type_name: PRODUCT_PROVIDER_DESCRIPTION_TYPE.to_owned(),
            fields: BTreeMap::from([("description".to_owned(), description)]),
        })
    }

    fn eval_product_provider_operand(
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
                                "product provider operand contains a non-byte element".to_owned(),
                            )
                        })
                })
                .collect(),
            other => Err(Halt::Trap(format!(
                "product provider operands must be byte data, got {other:?}"
            ))),
        }
    }
}
