//! Transitive admission for v1 inert opaque-representation carriers.

use diagnostics::Diagnostic;
use language_semantics::{DataSupplyMode, Multiplicity};
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataMember};
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

use representation_selections::OpaqueRepresentationCopyDisposition;

pub(super) fn validate_inert_carrier(
    program: &TypedTrees,
    opaque: &DataDefinition,
    carrier: TypeReferenceHandle,
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
        scopes: validation::DeclarationScopes::default(),
        applications: Vec::new(),
    }
    .validate_type(carrier, 0, &program.display_type_reference(carrier))?;
    Ok(copy_disposition)
}

struct CarrierClosure<'program> {
    program: &'program TypedTrees,
    copy_disposition: OpaqueRepresentationCopyDisposition,
    visiting: Vec<typed_trees::type_identity::TypeIdentityId>,
    validated: Vec<typed_trees::type_identity::TypeIdentityId>,
    scopes: validation::DeclarationScopes,
    applications: Vec<(SymbolHandle, TypeReferenceHandle, usize)>,
}

impl CarrierClosure<'_> {
    fn validate_definition(
        &mut self,
        definition: &DataDefinition,
        identity: typed_trees::type_identity::TypeIdentityId,
        scope: usize,
        path: &str,
    ) -> Result<(), Diagnostic> {
        if self.validated.contains(&identity) {
            return Ok(());
        }
        if self.visiting.contains(&identity) {
            return Err(self.reject(path, "contains a recursive by-value carrier cycle"));
        }
        if definition.supply_mode != DataSupplyMode::CheckedShape {
            return Err(self.reject(
                path,
                "contains boundary-opaque data without its own joined representation",
            ));
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

        self.visiting.push(identity);
        for member in self.program.data_members(definition) {
            match member {
                DataMember::Field(field) if !field.relevance.is_erased() => {
                    self.validate_type(
                        field.type_reference,
                        scope,
                        &format!("{path}.{}", field.name),
                    )?;
                }
                DataMember::Variant(variant) => {
                    for field in self.program.data_payload_fields(variant) {
                        if field.relevance.is_erased() {
                            continue;
                        }
                        self.validate_type(
                            field.type_reference,
                            scope,
                            &format!("{path}::{}({})", variant.name, field.name),
                        )?;
                    }
                }
                DataMember::Field(_) => {}
            }
        }
        let removed = self.visiting.pop();
        debug_assert_eq!(removed, Some(identity));
        self.validated.push(identity);
        Ok(())
    }

    fn validate_type(
        &mut self,
        type_reference: TypeReferenceHandle,
        scope: usize,
        path: &str,
    ) -> Result<(), Diagnostic> {
        let (type_reference, scope) = self.scopes.head(self.program, type_reference, scope);
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
                self.validate_type(*base_type, scope, path)
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                if !matches!(length, FixedArrayLength::Literal(_)) {
                    return Err(self.reject(path, "contains an unresolved array extent"));
                }
                self.validate_type(*element_type, scope, &format!("{path}[]"))
            }
            TypeReferenceNode::Named { symbol, .. }
                if self.program.symbols.get(*symbol).kind == SymbolKind::BuiltinType =>
            {
                Ok(())
            }
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                let identity = self
                    .scopes
                    .type_identity(self.program, type_reference, scope);
                let (definition, field_scope) = self
                    .scopes
                    .definition(self.program, type_reference, scope)
                    .ok_or_else(|| {
                        self.reject(path, "contains data without an exact closed application")
                    })?;
                // Entering an already supplied actual finishes that template's
                // expansion. Keep all other origins active: a different ancestor
                // may still be manufacturing a growing family through this field.
                let ancestors = std::mem::take(&mut self.applications);
                self.applications.extend(ancestors.iter().copied().filter(
                    |(_, parent, parent_scope)| {
                        !self.scopes.is_strict_argument(
                            self.program,
                            *parent,
                            *parent_scope,
                            identity,
                        )
                    },
                ));
                let result = if self
                    .applications
                    .iter()
                    .any(|(symbol, _, _)| *symbol == definition.symbol)
                {
                    Err(self.reject(path, "contains a recursive by-value carrier family"))
                } else {
                    self.applications
                        .push((definition.symbol, type_reference, scope));
                    self.validate_definition(definition, identity, field_scope, path)
                };
                self.applications = ancestors;
                result
            }
            TypeReferenceNode::Reference { .. } => {
                Err(self.reject(path, "contains borrowed or external storage"))
            }
            TypeReferenceNode::Slice { .. } => {
                Err(self.reject(path, "contains dynamically sized external storage"))
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
