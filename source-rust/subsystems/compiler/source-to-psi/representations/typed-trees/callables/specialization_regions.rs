//! Regions inside a supplied type are bound by the use of a specialization.
//! The shared body needs one formal per position, including two positions
//! whose actual regions happen to coincide at the first call.
use super::CallableApplication;
use crate::source_state::TypedTrees;
use crate::source_state::name::Identifier;
use crate::source_state::static_applications::{
    StaticMachineApplicationHandle, StaticMachineBindingValue,
};
use crate::source_state::type_identity::{
    LifetimeParameterIdentity, declaration_lifetime_parameters, lifetime_parameter_identity,
    lifetime_parameter_name,
};
use crate::source_state::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
use symbols::SymbolHandle;

pub(super) struct RegionUse {
    pub(super) reference: TypeReferenceHandle,
    pub(super) slot: usize,
    pub(super) name: Identifier,
    pub(super) region: LifetimeParameterIdentity,
}

fn operand_regions(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    supplier: SymbolHandle,
) -> Result<Vec<RegionUse>, &'static str> {
    let mut uses = Vec::new();
    let mut stack = vec![(reference, 0)];
    let table = &program.type_reference_table;
    while let Some((reference, depth)) = stack.pop() {
        if depth >= 256 || !table.contains_type_reference(reference) {
            return Err("specialized type operand contains an invalid type tree");
        }
        let (names, children) = match table.type_reference(reference) {
            TypeReferenceNode::Reference {
                lifetime, referee, ..
            } => (lifetime.as_slice(), vec![*referee]),
            TypeReferenceNode::Generic {
                lifetime_arguments,
                arguments,
                ..
            } => (
                lifetime_arguments.as_slice(),
                table.type_reference_handles(*arguments).to_vec(),
            ),
            TypeReferenceNode::Slice { element_type }
            | TypeReferenceNode::FixedArray { element_type, .. } => (&[][..], vec![*element_type]),
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let mut children = vec![*base_type];
                for constraint in table.constraints(*constraints) {
                    if let TypeConstraintNode::Domain(domain) = constraint {
                        children.extend_from_slice(&domain.arguments);
                    }
                }
                (&[][..], children)
            }
            _ => continue,
        };
        let owner = table.operand_lifetime_owner(reference);
        let owner = if owner.is_valid() { owner } else { supplier };
        for (slot, name) in names.iter().enumerate() {
            let supplied = table.operand_lifetimes(reference).get(slot).copied();
            let region = match supplied {
                Some(region) if region.declaration.is_valid() => region,
                Some(_) => continue, // Unmaterialized elision uses the ordinary signature rule.
                None => lifetime_parameter_identity(program, owner, name.as_str()).ok_or(
                    "specialized type argument has no declaration for its supplied lifetime",
                )?,
            };
            if lifetime_parameter_name(program, region) != Some(name) {
                return Err(
                    "specialized type argument's lifetime differs from its declaration ordinal",
                );
            }
            uses.push(RegionUse {
                reference,
                slot,
                name: name.clone(),
                region,
            });
        }
        stack.extend(children.into_iter().rev().map(|child| (child, depth + 1)));
    }
    Ok(uses)
}

impl TypedTrees {
    /// Replace each supplied lifetime occurrence by a fresh formal of this
    /// shared executable. Applying the ordinary type copier preserves the
    /// remaining type structure, constraints, origins, and obligations.
    pub fn generalize_static_type_regions(
        &mut self,
        reference: TypeReferenceHandle,
        supplier: SymbolHandle,
        instance: SymbolHandle,
        formals: &mut Vec<Identifier>,
    ) -> Result<TypeReferenceHandle, &'static str> {
        let mut sites = operand_regions(self, reference, supplier)?;
        if sites.is_empty() {
            return Ok(reference);
        }
        for site in &mut sites {
            let ordinal = formals.len();
            site.region = LifetimeParameterIdentity {
                declaration: instance,
                ordinal,
            };
            site.name = Identifier::generated(format!("@type-argument-region-{ordinal}"));
            formals.push(site.name.clone());
        }
        let mut application = CallableApplication {
            specialized_regions: sites.into(),
            region_scope: supplier,
            ..Default::default()
        };
        let generalized = application.type_reference(self, reference)?;
        if !application.specialized_regions.is_empty() {
            return Err("specialization did not bind every supplied type region");
        }
        Ok(generalized)
    }

    /// Regions of the original declaration keep its identity. Extra formals
    /// for static type operands belong to the generated executable.
    pub fn executable_lifetime_parameters(
        &self,
        owner: SymbolHandle,
    ) -> Vec<LifetimeParameterIdentity> {
        let declaration = self.lifetime_declaration_owner(owner);
        let declared = declaration_lifetime_parameters(self, declaration).map_or(0, <[_]>::len);
        let count = self.machine_by_symbol(owner).map_or(declared, |machine| {
            machine.lifetime_parameters.len().max(declared)
        });
        (0..count)
            .map(|ordinal| LifetimeParameterIdentity {
                declaration: if ordinal < declared {
                    declaration
                } else {
                    owner
                },
                ordinal,
            })
            .collect()
    }

    /// The same region positions that were generalized when creating the
    /// executable are now bound using this call's own retained type operands.
    pub fn specialization_type_argument_regions(
        &self,
        instance: SymbolHandle,
        application: StaticMachineApplicationHandle,
    ) -> Result<Vec<(LifetimeParameterIdentity, LifetimeParameterIdentity)>, &'static str> {
        let Some(specialization) = self.machine_specialization(instance) else {
            return Ok(Vec::new());
        };
        let formals = self.executable_lifetime_parameters(instance);
        let extra = formals
            .iter()
            .filter(|formal| formal.declaration == instance)
            .copied()
            .collect::<Vec<_>>();
        if extra.is_empty() {
            return Ok(Vec::new());
        }
        let table = &self.expression_table;
        if !table.static_machine_application_is_valid(application) {
            return Err(
                "a specialization with type-argument regions needs its invoking application",
            );
        }
        let source = table.static_machine_application(application);
        let original = table.static_machine_application(specialization.binding_application);
        if source.binding_declaration != original.binding_declaration {
            return Err("specialization region arguments name a different declaration");
        }
        let mut actual = Vec::new();
        for binding in table.static_machine_application_bindings(application) {
            if let StaticMachineBindingValue::Type(reference) = binding.value {
                actual.extend(
                    operand_regions(self, reference, source.lexical_owner)?
                        .into_iter()
                        .map(|site| site.region),
                );
            }
        }
        if actual.len() != extra.len() {
            return Err("specialization has another set of type-argument region positions");
        }
        Ok(extra.into_iter().zip(actual).collect())
    }
}
