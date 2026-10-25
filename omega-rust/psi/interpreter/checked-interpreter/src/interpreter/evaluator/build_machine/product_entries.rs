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
use typed_trees::product_operand::ProductEntryDeclaration;
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
        // retained product dependencies and selects a `public` declaration in
        // that exact target package's product checked instance. A helper keeps
        // its operational capability but cannot enumerate the caller's product
        // namespace, and a build-scope alias authorizes nothing. A bare leaf
        // keeps the same-package declaration scan; a first segment that is
        // no product alias leaves only the occurrence's own package path
        // reading of the full spelling.
        let qualified = name.split_once("::");
        let authorized = qualified.and_then(|(alias, _)| {
            self.product_declarations
                .symbols
                .product_dependency_target(occurrence, alias)
        });
        let candidates = match (qualified, authorized) {
            (Some((_, rest)), Some(target)) => self
                .product_declarations
                .machines()
                .iter()
                .filter(|machine| {
                    machine.is_public
                        // The dependency alias qualifies a complete path;
                        // a different module's relative name is not a match.
                        && self.product_declarations.symbols.display_path(machine.symbol, "::") == rest
                        && self
                            .product_declarations
                            .symbols
                            .symbol_product_package_identity(machine.symbol)
                            == Some(target)
                })
                .collect::<Vec<_>>(),
            _ => self
                .product_declarations
                .machines()
                .iter()
                .filter(|machine| {
                    (match qualified {
                        Some(_) => {
                            self.product_declarations
                                .symbols
                                .display_path(machine.symbol, "::")
                                .as_str()
                                == name
                        }
                        None => machine.name.as_str() == name,
                    }) && self
                        .product_declarations
                        .symbols
                        .symbol_source_span(machine.symbol)
                        .is_some_and(|span| {
                            self.product_declarations
                                .symbols
                                .same_product_package_instance(occurrence, span)
                        })
                })
                .collect::<Vec<_>>(),
        };
        if candidates.is_empty() {
            // The probe spelling is the declaration part of the path: for a
            // qualified query it is the segment after the resolved or
            // unrecognized alias.
            let probe = qualified.map_or(name, |(_, rest)| rest);
            let message = if self.product_declarations.machines().iter().any(|machine| {
                machine.name.as_str() == probe
                    || self
                        .product_declarations
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
        }
        let compatibility = self.product_entry_compatibility.ok_or_else(|| {
            Halt::Trap(
                "product entry selection requires the build owner's slot compatibility check"
                    .to_owned(),
            )
        })?;
        // Visibility is settled before signature filtering, so a query cannot
        // expose an inaccessible declaration through compatibility diagnostics.
        // The expected slot selects among authorized candidates; declaration
        // order and equal names never choose an implementation.
        let mut selected = None;
        let mut rejection = None;
        for candidate in candidates {
            let declaration =
                ProductEntryDeclaration::from_machine(self.product_declarations, candidate);
            match compatibility.validate_entry(self.product_declarations, declaration, slot) {
                Ok(()) => {
                    if selected.is_some() {
                        return Err(Halt::Trap(format!(
                            "product entry `{name}` is ambiguous: more than one visible declaration is compatible with slot `{slot}`"
                        )));
                    }
                    selected = Some(candidate);
                }
                Err(reason) => {
                    rejection.get_or_insert(reason);
                }
            }
        }
        let machine = selected.ok_or_else(|| {
            Halt::Trap(format!(
                "product entry `{name}` is incompatible with slot `{slot}`: {}",
                rejection.as_deref().unwrap_or("no compatible declaration")
            ))
        })?;
        let index = self.product_entry_descriptions.len();
        self.product_entry_descriptions
            .try_reserve(1)
            .map_err(|_| {
                Halt::Resource("product-entry description allocation was refused".to_owned())
            })?;
        self.product_entry_descriptions.push(DescribedProductEntry {
            machine_symbol: machine.symbol,
            machine_name: machine.name.as_str().to_owned(),
            slot: slot.to_owned(),
        });
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
