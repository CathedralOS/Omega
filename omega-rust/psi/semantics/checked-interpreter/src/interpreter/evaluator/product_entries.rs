//! Compiler-owned product-entry descriptions (`Build.product.entry`).
//!
//! The query is a designated compiler operation, not an ordinary call: it
//! resolves one exact product machine under the query occurrence's lexical
//! package scope -- its own package, or a `pub` declaration inside a package
//! the occurrence's package holds an authorized product dependency on -- and
//! hands back an opaque `ProductEntryRef` marker. The
//! semantic payload stays in the evaluator's private
//! `product_entry_descriptions` table, so evaluated code can carry the marker
//! but cannot read, convert, or fabricate the selection. No product receiver,
//! body, initializer, or provider executes -- the query is a logical lookup
//! over the authored frontier.

use super::{BTreeMap, EvalResult, Evaluator, ExpressionHandle, Frame, Halt, SymbolHandle, Value};
/// The compiler-created `Build.product` facet value inside the canonical
/// Build activation. An authored `BuildProduct {}` has the same static shape
/// but never this marker name, so it cannot perform the query.
pub(super) const PRODUCT_FACET_TYPE: &str = "$OmegaBuildProductFacet";

/// The marker type name carried by issued `ProductEntryRef` values. An
/// authored `ProductEntryRef {}` has the declared type name instead and is
/// refused by `described_product_entry` below.
const PRODUCT_ENTRY_DESCRIPTION_TYPE: &str = "$OmegaBuildProductEntry";

impl<'program> Evaluator<'program> {
    /// `builder.product.entry(path, slot)` as a value-position call: resolve
    /// `path` under the CALL's lexical package scope and return an opaque
    /// description marker.
    pub(super) fn try_build_product_entry_value_call(
        &mut self,
        handle: ExpressionHandle,
        call: &typed_trees::expression::TableCallExpression,
        frame: &Frame,
    ) -> EvalResult<Option<Value>> {
        if call.target.as_str() != "entry" || !call.receiver.is_valid() {
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
                && self.exact_build_facet_method("BuildProduct", "entry", call.target_symbol)
            {
                return Err(Halt::Trap(
                    "product entry selection requires the compiler-issued Build.product facet"
                        .to_owned(),
                ));
            }
            return Ok(None);
        }
        if !self.exact_build_facet_method("BuildProduct", "entry", call.target_symbol) {
            return Err(Halt::Trap(
                "product entry selection did not select the exact toolchain machine".to_owned(),
            ));
        }
        let occurrence = self.program.expression_table.source_span(handle);
        let arguments = self
            .program
            .expression_table
            .expression_handles(call.arguments);
        let [path, slot] = arguments else {
            return Err(Halt::Trap(
                "product entry selection requires a path and a slot".to_owned(),
            ));
        };
        let path = self.eval_product_entry_operand(*path, frame)?;
        let slot = self.eval_product_entry_operand(*slot, frame)?;
        self.issue_product_entry_description(&path, &slot, occurrence)
            .map(Some)
    }

    /// The statement-position twin: `builder.product.entry(path, slot);`
    /// still performs and checks the query, then drops the description.
    pub(super) fn try_build_product_entry_statement(
        &mut self,
        call: &typed_trees::statement::TableCall,
        frame: &Frame,
    ) -> EvalResult<bool> {
        if call.target.as_str() != "entry" || call.receiver.is_empty() {
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
                && self.exact_build_facet_method("BuildProduct", "entry", call.target_symbol)
            {
                return Err(Halt::Trap(
                    "product entry selection requires the compiler-issued Build.product facet"
                        .to_owned(),
                ));
            }
            return Ok(false);
        }
        if !self.exact_build_facet_method("BuildProduct", "entry", call.target_symbol) {
            return Err(Halt::Trap(
                "product entry selection did not select the exact toolchain machine".to_owned(),
            ));
        }
        let arguments = self
            .program
            .statement_table
            .expression_handles(call.arguments);
        let [path, slot] = arguments else {
            return Err(Halt::Trap(
                "product entry selection requires a path and a slot".to_owned(),
            ));
        };
        let path = self.eval_product_entry_operand(*path, frame)?;
        let slot = self.eval_product_entry_operand(*slot, frame)?;
        self.issue_product_entry_description(&path, &slot, call.source_span)?;
        Ok(true)
    }

    /// Rejoin a `ProductEntryRef` runtime value to the description the
    /// compiler issued for it. Only the evaluator's own marker shape is
    /// admitted: an authored `ProductEntryRef {}`, a foreign struct, or a
    /// description index outside the issued table cannot satisfy this check.
    pub(super) fn described_product_entry(
        &self,
        value: &Value,
    ) -> EvalResult<crate::DescribedProductEntry> {
        let Value::Struct {
            type_name, fields, ..
        } = value
        else {
            return Err(Halt::Trap(
                "root binding operand is not a compiler-issued product entry description"
                    .to_owned(),
            ));
        };
        if type_name != PRODUCT_ENTRY_DESCRIPTION_TYPE {
            return Err(Halt::Trap(
                "root binding operand is not a compiler-issued product entry description"
                    .to_owned(),
            ));
        }
        let index = fields
            .get("description")
            .and_then(|cell| cell.borrow().as_int())
            .and_then(|index| usize::try_from(index).ok())
            .ok_or_else(|| {
                Halt::Trap(
                    "product entry description carries no valid description index".to_owned(),
                )
            })?;
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
        let name = std::str::from_utf8(path)
            .map_err(|_| Halt::Trap("product entry path must be canonical UTF-8".to_owned()))?;
        let slot = std::str::from_utf8(slot)
            .map_err(|_| Halt::Trap("product entry slot must be canonical UTF-8".to_owned()))?;
        if name.is_empty() || slot.is_empty() {
            return Err(Halt::Trap(
                "product entry selection requires a non-empty path and slot".to_owned(),
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
                .machines()
                .iter()
                .filter(|machine| {
                    machine.is_public
                        && (machine.name.as_str() == rest
                            || self
                                .program
                                .symbols
                                .display_path(machine.symbol, "::")
                                .as_str()
                                == rest)
                        && self.program.symbols.symbol_package_identity(machine.symbol)
                            == Some(target)
                })
                .collect::<Vec<_>>(),
            _ => self
                .program
                .machines()
                .iter()
                .filter(|machine| {
                    (match qualified {
                        Some(_) => {
                            self.program
                                .symbols
                                .display_path(machine.symbol, "::")
                                .as_str()
                                == name
                        }
                        None => machine.name.as_str() == name,
                    }) && self
                        .program
                        .symbols
                        .symbol_source_span(machine.symbol)
                        .is_some_and(|span| {
                            self.program
                                .symbols
                                .same_product_package_instance(occurrence, span)
                        })
                })
                .collect::<Vec<_>>(),
        };
        let [machine] = candidates.as_slice() else {
            if !candidates.is_empty() {
                return Err(Halt::Trap(format!(
                    "product entry `{name}` is ambiguous within its package"
                )));
            }
            // The probe spelling is the declaration part of the path: for a
            // qualified query it is the segment after the resolved or
            // unrecognized alias.
            let probe = qualified.map_or(name, |(_, rest)| rest);
            let message = if self.program.machines().iter().any(|machine| {
                machine.name.as_str() == probe
                    || self
                        .program
                        .symbols
                        .display_path(machine.symbol, "::")
                        .as_str()
                        == probe
            }) {
                format!(
                    "product entry `{name}` is not a product declaration visible from this build occurrence's package"
                )
            } else {
                format!(
                    "product entry `{name}` names no machine in this build occurrence's package"
                )
            };
            return Err(Halt::Trap(message));
        };
        let index = self.product_entry_descriptions.len();
        self.product_entry_descriptions
            .try_reserve(1)
            .map_err(|_| {
                Halt::Resource("product-entry description allocation was refused".to_owned())
            })?;
        self.product_entry_descriptions
            .push(crate::DescribedProductEntry {
                machine_symbol: machine.symbol,
                machine_name: machine.name.as_str().to_owned(),
                slot: slot.to_owned(),
            });
        let index = i64::try_from(index)
            .map_err(|_| Halt::Resource("product-entry description index overflowed".to_owned()))?;
        let description = self.allocate_cell(Value::Int(index))?;
        Ok(Value::Struct {
            type_symbol: SymbolHandle::invalid(),
            type_name: PRODUCT_ENTRY_DESCRIPTION_TYPE.to_owned(),
            fields: BTreeMap::from([("description".to_owned(), description)]),
        })
    }

    fn eval_product_entry_operand(
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
                                "product entry operand contains a non-byte element".to_owned(),
                            )
                        })
                })
                .collect(),
            other => Err(Halt::Trap(format!(
                "product entry operands must be byte data, got {other:?}"
            ))),
        }
    }
}
