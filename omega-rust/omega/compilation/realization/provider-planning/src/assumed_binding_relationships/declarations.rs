//! Exact public declaration views. No implementation body contributes a fact.

use crate::ProviderPlanningContext;
use crate::assumed_binding_registrations::ProductBoundaryRequirementReference;
use crate::calling_policy_plans::{
    MaterializedBoundarySignature, call_signature_from_top_level_requirement,
    call_signature_from_typed,
};
use checked_trees::CheckedTrees;
use checked_trees::typed_trees::name::Identifier;
use checked_trees::typed_trees::signature::{StateParameter, StateSignature};
use checked_trees::typed_trees::types::TypeReferenceHandle;
use diagnostics::Diagnostic;
use effects::provider_plan::{ServiceMethod, ServiceSchema};
use representation_planning::OpaqueRepresentationSelection;
use symbols::SymbolHandle;
use target::NativeTarget;

#[derive(Clone, Copy)]
pub(super) enum PublicRequirement<'program> {
    Trait {
        definition: &'program checked_trees::typed_trees::trait_definition::TraitDefinition,
        signature: &'program StateSignature,
    },
    Machine {
        declaration: &'program checked_trees::typed_trees::machine::Machine,
        entry: &'program checked_trees::typed_trees::state::State,
    },
}

pub(super) struct DeclaredRequirement<'program> {
    pub(super) application: &'program ProductBoundaryRequirementReference,
    pub(super) declaration: PublicRequirement<'program>,
    // Method identities remain with their original schema issuing table.
    pub(super) schema: ServiceSchema,
    pub(super) parameters: &'program [StateParameter],
    pub(super) result: TypeReferenceHandle,
    pub(super) lifetimes: Vec<&'program Identifier>,
}

pub(super) fn requirement<'program>(
    context: ProviderPlanningContext<'program>,
    reference: &'program ProductBoundaryRequirementReference,
) -> Result<DeclaredRequirement<'program>, Diagnostic> {
    let symbol = reference.symbol;
    if !symbol.is_valid() || !context.contains(symbol) {
        return Err(Diagnostic::error(
            "assumed binding requirement lost its exact Product declaration custody",
        ));
    }
    let typed = context.typed;
    // Application payload handles stay in their original frozen owner. Rejoin
    // the exact declaration's source prefix, never a same-number arena slot.
    let symbol =
        checked_trees::typed_trees::product_requirements::prepared_product_requirement_symbol(
            typed,
            &reference.application_demand(),
        )?;
    if let Some(declaration) = typed.machine(symbol) {
        let schema = crate::service_schema::from_typed_boundary_requirement(context, declaration)
            .ok_or_else(|| {
            Diagnostic::error(
                "assumed binding operand is not one exact bodyless boundary requirement",
            )
        })?;
        let [entry] = typed.machine_states(declaration) else {
            return Err(Diagnostic::error(
                "assumed binding requirement needs one public entry",
            ));
        };
        let [method] = schema.methods.as_slice() else {
            return Err(Diagnostic::error(
                "assumed binding requirement lost its public schema row",
            ));
        };
        if schema.identity_text(method.requirement_identity).is_empty() {
            return Err(Diagnostic::error(
                "assumed binding requirement lost its public identity owner",
            ));
        }
        return Ok(DeclaredRequirement {
            application: reference,
            declaration: PublicRequirement::Machine { declaration, entry },
            schema,
            parameters: typed.state_parameters(entry),
            result: entry.return_type,
            lifetimes: declaration.lifetime_parameters.iter().collect(),
        });
    }
    let (definition, signature) = typed.trait_machine_signature(symbol).ok_or_else(|| {
        Diagnostic::error(
            "assumed binding operand does not retain one exact public boundary signature",
        )
    })?;
    let schema =
        crate::service_schema::from_typed_requirement(context, symbol).ok_or_else(|| {
            Diagnostic::error("assumed binding signature lost its exact public schema row")
        })?;
    let [method] = schema.methods.as_slice() else {
        return Err(Diagnostic::error(
            "assumed binding signature lost its exact public schema row",
        ));
    };
    if schema.identity_text(method.requirement_identity).is_empty() {
        return Err(Diagnostic::error(
            "assumed binding signature lost its public identity owner",
        ));
    }
    Ok(DeclaredRequirement {
        application: reference,
        declaration: PublicRequirement::Trait {
            definition,
            signature,
        },
        schema,
        parameters: typed.state_signature_parameters(signature),
        result: signature.return_type,
        lifetimes: definition
            .lifetime_parameters
            .iter()
            .chain(&signature.lifetime_parameters)
            .collect(),
    })
}

impl DeclaredRequirement<'_> {
    pub(super) fn own_service(&self) -> SymbolHandle {
        match self.declaration {
            PublicRequirement::Trait { definition, .. } => definition.symbol,
            PublicRequirement::Machine { .. } => SymbolHandle::invalid(),
        }
    }

    pub(super) fn service_reach(
        &self,
        program: &CheckedTrees,
    ) -> Result<Vec<SymbolHandle>, Diagnostic> {
        let row = match self.declaration {
            PublicRequirement::Trait { signature, .. } => signature.service_reach_row,
            PublicRequirement::Machine { declaration, .. } => declaration.service_reach_row,
        };
        let mut services = program.service_reach_rows.services(row).to_vec();
        if self.own_service().is_valid() {
            let own = program.service_reaches.id_for_symbol(self.own_service()).ok_or_else(|| Diagnostic::error(
                "assumed binding public requirement lost its exact boundary service identity",
            ))?;
            program.service_reaches.extend_closure(own, &mut services);
        }
        let mut symbols = Vec::with_capacity(services.len());
        for service in services {
            let definition = program.service_reaches.definition(service).ok_or_else(|| {
                Diagnostic::error("assumed binding public reach lost its exact service declaration")
            })?;
            if !definition.symbol.is_valid() {
                return Err(Diagnostic::error(
                    "assumed binding public reach contains an absent service identity",
                ));
            }
            if !symbols.contains(&definition.symbol) {
                symbols.push(definition.symbol);
            }
        }
        Ok(symbols)
    }

    pub(super) fn invocations(
        &self,
        program: &CheckedTrees,
    ) -> Vec<flow_effects::InvocationTarget> {
        match self.declaration {
            PublicRequirement::Trait { signature, .. } => {
                checked_trees::typed_trees::invocations::declared_signature_invocations(
                    program, signature,
                )
            }
            PublicRequirement::Machine { declaration, .. } => {
                checked_trees::typed_trees::invocations::declared_machine_invocations(
                    program,
                    declaration,
                )
            }
        }
    }

    pub(super) fn symbol(&self) -> SymbolHandle {
        match self.declaration {
            PublicRequirement::Trait { signature, .. } => signature.symbol,
            PublicRequirement::Machine { declaration, .. } => declaration.symbol,
        }
    }

    pub(super) fn method(&self) -> &ServiceMethod {
        &self.schema.methods[0]
    }

    pub(super) fn requirement_identity(&self) -> &str {
        self.schema
            .identity_text(self.method().requirement_identity)
    }

    pub(super) fn public_contract(&self, program: &CheckedTrees) -> Result<[u8; 32], Diagnostic> {
        match self.declaration {
            PublicRequirement::Trait { signature, .. } => {
                let mut encoder = crate::digest_encoder::DomainDigestEncoder::new(
                    b"omega.assumed-binding-public-signature.v1\0",
                );
                encoder.bytes(
                    program.facts.public_contract_sources.signature_bytes(signature.symbol)
                        .ok_or_else(|| Diagnostic::error(
                            "assumed binding signature lost its original checked public content",
                        ))?,
                );
                // Signature bytes retain the scalar/public declaration
                // projection; the genuine published capsule also commits the
                // invocation and crash axes. Neither is a body envelope.
                let capsule = self.crash_capsule(program).ok_or_else(|| {
                    Diagnostic::error(
                        "assumed binding signature lost its checked public contract capsule",
                    )
                })?;
                if capsule.target_contract_commitment().is_zero() {
                    return Err(Diagnostic::error(
                        "assumed binding public signature has no checked contract commitment",
                    ));
                }
                encoder.raw(&capsule.target_contract_commitment().as_bytes());
                Ok(encoder.finish())
            }
            PublicRequirement::Machine { declaration, .. } => {
                // The existing checked declaration commitment includes machine
                // and state clauses. A fabricated StateSignature would omit
                // machine contracts or invent an arena join between the two.
                program.facts.contract_plans.for_machine(declaration.symbol)
                    .map(|plan| plan.commitment.as_bytes())
                    .filter(|digest| *digest != [0; 32])
                    .ok_or_else(|| Diagnostic::error(
                        "assumed binding requirement lost its checked public declaration contract",
                    ))
            }
        }
    }

    pub(super) fn materialize(
        &self,
        context: ProviderPlanningContext<'_>,
        target: NativeTarget,
        opaque_representations: &[OpaqueRepresentationSelection],
    ) -> Result<MaterializedBoundarySignature, Diagnostic> {
        let materialized = match self.declaration {
            PublicRequirement::Trait { signature, .. } => call_signature_from_typed(
                context.typed,
                context
                    .typed
                    .public_trait_requirement_signature(signature.symbol)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "assumed Calling signature lost its original public declaration",
                        )
                    })?,
                &[],
                self.requirement_identity(),
                target,
                opaque_representations,
            ),
            PublicRequirement::Machine { declaration, entry } => {
                call_signature_from_top_level_requirement(
                    context.typed,
                    declaration,
                    entry,
                    self.requirement_identity(),
                    target,
                    opaque_representations,
                )
            }
        };
        materialized.map_err(Diagnostic::error)
    }

    pub(super) fn crash_capsule<'program>(
        &self,
        program: &'program CheckedTrees,
    ) -> Option<&'program checked_trees::CrashContractCapsule> {
        let (machine, state) = match self.declaration {
            PublicRequirement::Trait {
                definition,
                signature,
            } => (definition.symbol, signature.symbol),
            PublicRequirement::Machine { declaration, entry } => (declaration.symbol, entry.symbol),
        };
        program.facts.contract_plans.crash_capsule(machine, state)
    }
}

/// Original public row named by the selected plan, before source application.
/// Its signature is compared with the genuine template row; the produced
/// instance independently owns the applied runtime signature and contracts.
pub(super) fn authored_schema(
    context: ProviderPlanningContext<'_>,
    symbol: SymbolHandle,
) -> Result<ServiceSchema, Diagnostic> {
    if let Some(machine) = context.typed.machine(symbol) {
        let schema = crate::service_schema::from_typed_boundary_requirement(context, machine)
            .ok_or_else(|| {
                Diagnostic::error("selected application lost its original requirement schema")
            })?;
        let [method] = schema.methods.as_slice() else {
            return Err(Diagnostic::error(
                "selected application lost its original public method",
            ));
        };
        if schema.identity_text(method.requirement_identity).is_empty() {
            return Err(Diagnostic::error(
                "selected application lost its public identity owner",
            ));
        }
        return Ok(schema);
    }
    let (definition, signature) =
        context
            .typed
            .trait_machine_signature(symbol)
            .ok_or_else(|| {
                Diagnostic::error("selected application lost its original signature owner")
            })?;
    let mut schema = crate::service_schema::from_typed(context, &[], definition)
        .ok_or_else(|| Diagnostic::error("selected application lost its original trait schema"))?;
    let identity = context
        .typed
        .trait_requirement_callable_identity_text(definition, signature);
    let identity = schema
        .identities
        .id_at(&identity)
        .ok_or_else(|| Diagnostic::error("selected application lost its public trait method"))?;
    schema
        .methods
        .retain(|method| method.requirement_identity == identity);
    match schema.methods.as_slice() {
        [] => Err(Diagnostic::error(
            "selected application lost its public trait method",
        )),
        [_] => Ok(schema),
        _ => Err(Diagnostic::error(
            "selected application has competing public trait methods",
        )),
    }
}
