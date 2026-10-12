//! Copy captured carrier payloads beside their original lexical namespace.
//!
//! A Product operand borrows its exact Build binder roster but owns a different
//! type/expression arena. Reuse the complete type copier before publishing each
//! carrier receipt; regions still refer to the original immutable capture.
//! Shared source custody and exact declaration coordinates permit local copies,
//! not portable admission or a new declaration selected by its display path.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::references::TypedReferences;
use typed_trees::typed_trees::product_applications::{
    CapturedStaticLexicalScope, CapturedStaticLexicalScopeSlot, CapturedStaticValueCarrier,
};

pub(crate) fn retain(
    program: &mut TypedTrees,
    scope: &CapturedStaticLexicalScope,
) -> Result<arena::Handle<CapturedStaticLexicalScopeSlot>, Diagnostic> {
    let handle = program.retain_captured_static_lexical_scope(scope);
    if !program.begin_captured_static_value_carriers(handle) {
        return Ok(handle);
    }
    let source = scope.owner();
    let mut carriers = Vec::new();
    for parameter in scope.type_parameters() {
        let original_carrier = match parameter.kind {
            typed_trees::data::TypeParameterKind::Const { type_reference }
            | typed_trees::data::TypeParameterKind::Value { type_reference } => type_reference,
            _ => continue,
        };
        if !source
            .type_reference_table
            .contains_type_reference(original_carrier)
        {
            continue;
        }
        let mut namespace = CarrierNamespace {
            source,
            receiving: program,
            available: true,
        };
        typed_trees::references::visit_type_reference(source, &mut namespace, original_carrier);
        if !namespace.available {
            continue;
        }
        let carrier = super::body_cloning::copy_type_reference_payloads(
            Some(source),
            program,
            original_carrier,
            &[],
        )?;
        carriers.push(CapturedStaticValueCarrier {
            parameter: parameter.symbol,
            original_carrier,
            carrier,
        });
    }
    program.retain_captured_static_value_carriers(handle, carriers);
    Ok(handle)
}

struct CarrierNamespace<'program> {
    source: &'program TypedTrees,
    receiving: &'program TypedTrees,
    available: bool,
}

impl Extend<SymbolHandle> for CarrierNamespace<'_> {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
        for symbol in symbols {
            // The reference roster includes ZII metadata and canonical value
            // atoms with no declaration coordinate. Their payload/Meaning
            // admission remains in the normalizer, not this namespace join.
            if !symbol.is_valid() {
                continue;
            }
            self.available &= self
                .receiving
                .retains_source_declaration_symbol(self.source, symbol);
        }
    }
}

impl TypedReferences for CarrierNamespace<'_> {
    fn type_reference(&mut self, reference: typed_trees::types::TypeReferenceHandle) -> bool {
        let origin = self
            .source
            .type_reference_table
            .data_application_origin(reference);
        if origin.is_valid() {
            // Identity consumes this exact per-use source application before
            // generated nominal lookup. The copier retains that same graph;
            // another destination instance cannot donate its first tuple.
            let source = self.source;
            typed_trees::references::visit_type_reference(source, self, origin);
            false
        } else {
            true
        }
    }
}
