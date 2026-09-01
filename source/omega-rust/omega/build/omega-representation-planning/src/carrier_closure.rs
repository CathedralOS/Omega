//! Transitive admission for v1 inert opaque-representation carriers.

use psi_diagnostics::Diagnostic;
use psi_language_semantics::{DataSupplyMode, Multiplicity};
use psi_symbols::{SymbolHandle, SymbolKind};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{DataDefinition, DataMember};
use psi_typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

use omega_representation_selections::OpaqueRepresentationCopyDisposition;

pub(super) fn validate_inert_carrier(
    program: &TypedTrees,
    opaque: &DataDefinition,
    carrier: &DataDefinition,
) -> Result<OpaqueRepresentationCopyDisposition, Diagnostic> {
    let copy_disposition = if opaque.properties.multiplicity == Multiplicity::Unrestricted {
        OpaqueRepresentationCopyDisposition::CheckedSemanticCopy
    } else {
        OpaqueRepresentationCopyDisposition::PlacementOnly
    };
    CarrierClosure {
        program,
        copy_disposition,
        visiting: Vec::new(),
        validated: Vec::new(),
    }
    .validate_definition(carrier, carrier.name.as_str())?;
    Ok(copy_disposition)
}

struct CarrierClosure<'program> {
    program: &'program TypedTrees,
    copy_disposition: OpaqueRepresentationCopyDisposition,
    visiting: Vec<SymbolHandle>,
    validated: Vec<SymbolHandle>,
}

impl CarrierClosure<'_> {
    fn validate_definition(
        &mut self,
        definition: &DataDefinition,
        path: &str,
    ) -> Result<(), Diagnostic> {
        if self.validated.contains(&definition.symbol) {
            return Ok(());
        }
        if self.visiting.contains(&definition.symbol) {
            return Err(self.reject(path, "contains a recursive by-value carrier cycle"));
        }
        if definition.supply_mode != DataSupplyMode::CheckedShape {
            return Err(self.reject(
                path,
                "contains boundary-opaque data without its own joined representation",
            ));
        }
        if !self.program.data_type_parameters(definition).is_empty()
            || !definition.lifetime_parameters.is_empty()
        {
            return Err(self.reject(path, "contains a declaration that is not target-closed"));
        }
        if definition.properties.multiplicity == Multiplicity::Linear {
            return Err(self.reject(path, "contains live linear ownership debt"));
        }
        if self.copy_disposition == OpaqueRepresentationCopyDisposition::CheckedSemanticCopy
            && definition.properties.multiplicity != Multiplicity::Unrestricted
        {
            return Err(self.reject(
                path,
                "is affine; a copyable opaque datum requires every carrier declaration to be structurally `[copy]`",
            ));
        }
        if self.program.machines().iter().any(|machine| {
            machine.attached_data_symbol == definition.symbol
                && machine.name.as_str().rsplit("::").next() == Some("drop")
        }) {
            return Err(self.reject(path, "contains independently invoked nominal cleanup"));
        }

        self.visiting.push(definition.symbol);
        for member in self.program.data_members(definition) {
            match member {
                DataMember::Field(field) if !field.relevance.is_erased() => {
                    self.validate_type(field.type_reference, &format!("{path}.{}", field.name))?;
                }
                DataMember::Variant(variant) => {
                    for field in self.program.data_payload_fields(variant) {
                        if field.relevance.is_erased() {
                            continue;
                        }
                        self.validate_type(
                            field.type_reference,
                            &format!("{path}::{}({})", variant.name, field.name),
                        )?;
                    }
                }
                DataMember::Field(_) => {}
            }
        }
        let removed = self.visiting.pop();
        debug_assert_eq!(removed, Some(definition.symbol));
        self.validated.push(definition.symbol);
        Ok(())
    }

    fn validate_type(
        &mut self,
        type_reference: TypeReferenceHandle,
        path: &str,
    ) -> Result<(), Diagnostic> {
        if self
            .program
            .primitive_type_reference(type_reference)
            .is_some()
        {
            return Ok(());
        }
        match self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            TypeReferenceNode::Constrained { base_type, .. } => {
                self.validate_type(*base_type, path)
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                if !matches!(length, FixedArrayLength::Literal(_)) {
                    return Err(self.reject(path, "contains an unresolved array extent"));
                }
                self.validate_type(*element_type, &format!("{path}[]"))
            }
            TypeReferenceNode::Named { symbol, .. }
                if self.program.symbols.get(*symbol).kind == SymbolKind::BuiltinType =>
            {
                Ok(())
            }
            TypeReferenceNode::Named { symbol, .. } => {
                let definition = self
                    .program
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.symbol == *symbol)
                    .ok_or_else(|| {
                        self.reject(path, "contains data without an exact checked declaration")
                    })?;
                self.validate_definition(definition, path)
            }
            TypeReferenceNode::Reference { .. } => {
                Err(self.reject(path, "contains borrowed or external storage"))
            }
            TypeReferenceNode::Slice { .. } => {
                Err(self.reject(path, "contains dynamically sized external storage"))
            }
            TypeReferenceNode::Generic { .. } => {
                Err(self.reject(path, "contains a generic carrier that is not target-closed"))
            }
            TypeReferenceNode::DynamicTrait { .. } => {
                Err(self.reject(path, "contains dynamic external provider state"))
            }
            TypeReferenceNode::ConstExpression(_) => Err(self.reject(
                path,
                "contains a proof-static expression as runtime storage",
            )),
            TypeReferenceNode::Unit => Ok(()),
        }
    }

    fn reject(&self, path: &str, reason: &str) -> Diagnostic {
        Diagnostic::error(format!(
            "opaque representation carrier graph at `{path}` {reason}; v1 carriers must be inert storage"
        ))
    }
}
