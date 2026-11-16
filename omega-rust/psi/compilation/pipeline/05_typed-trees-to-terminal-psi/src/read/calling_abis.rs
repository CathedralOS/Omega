//! Declaration ABI metadata belongs to the exact boundaries already read and
//! any additional nominated declarations. It adds no calls or executable roots.

use std::sync::Arc;

use language_semantics::callback_declarations::BoundaryTelescope;
use semantic_vocabulary::{CallingAbiIdentity, CallingSignatureId, MachineId, StructuralTypeId};
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason};
use terminal_psi::calling_policies::*;
use terminal_psi::{StructuralAccess, TerminalIdentityTableBuilder, TerminalModule};
use typed_trees::service_declarations::provider_declarations::ProviderPlanningContext;
use typed_trees::service_declarations::{PreparedServiceCatalog, ProviderSchemaDeclaration};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use super::Program;

pub(crate) struct PreparedCallingAbis {
    entry: MachineId,
    declarations: Vec<PreparedAbi>,
    boundary_applications: Vec<terminal_psi::CallingSignatureDeclaration>,
}

struct PreparedAbi {
    identity: CallingAbiIdentity,
    formals: Vec<semantic_vocabulary::CallingGenericFormal>,
    default_eligible: bool,
    physical_owner: Result<semantic_vocabulary::CallingDeclarationOwner, String>,
    storage: Result<PreparedStorage, String>,
}

#[derive(PartialEq, Eq)]
struct PreparedStorage {
    parameters: Vec<CallingAbiParameter>,
    result: Option<StructuralTypeId>,
    telescope: BoundaryTelescope,
}

fn unavailable(entry: MachineId, detail: impl Into<String>) -> Refusal {
    Refusal {
        reason: RefusalReason::Unsupported,
        machine: entry,
        concerns: Construct::Machine,
        source_span: None,
        detail: detail.into(),
    }
}

impl Program<'_> {
    /// Read the ABI applications of this module's boundaries and exact nominations.
    /// Storage is registered before lowering freezes the structural namespace.
    /// An unsupported storage shape remains an explicit per-declaration refusal;
    /// requesting metadata does not widen the executable body's feature set.
    pub(crate) fn prepare_calling_abis(
        &mut self,
        entry: MachineId,
        boundaries: &[SymbolHandle],
    ) -> Checked<PreparedCallingAbis> {
        if self.type_purpose != super::storage_metadata::TypePurpose::StorageDescription {
            return Err(unavailable(
                entry,
                "calling ABI storage requested before the complete body closure",
            ));
        }
        if boundaries.is_empty() && self.boundaries.rows.is_empty() {
            return Ok(PreparedCallingAbis {
                entry,
                declarations: Vec::new(),
                boundary_applications: Vec::new(),
            });
        }
        let typed = self.typed;
        let context = ProviderPlanningContext::new(typed, self.purpose);
        // This catalog supplies source signatures, not selection authority.
        // The coordinator authenticates the nominated handles against its
        // admitted source registry before constructing this compilation.
        let sources =
            source::source_identity::toolchain_source_identities(typed.symbols.source_files())
                .map_err(|reason| unavailable(entry, reason))?;
        let catalog = Arc::new(PreparedServiceCatalog::prepare(context, &sources));
        let services = catalog
            .bind(context)
            .map_err(|error| unavailable(entry, format!("calling metadata catalog: {error:?}")))?;
        let signatures = services.signatures();
        let mut ids = Vec::<CallingSignatureId>::new();
        let mut add = |id| {
            if !ids.contains(&id) {
                ids.push(id);
            }
        };
        // Every boundary already read into this module owns its declaration
        // ABI, including uncalled members of a supplied slot. An open schema
        // keeps its reason for lacking a physical application; reading its
        // declaration does not choose static arguments for it.
        for (_, boundary) in self.boundaries.rows.iter() {
            if let Some(Ok(application)) = signatures.abi_for_boundary(boundary.source) {
                add(*application);
            }
        }
        for symbol in boundaries {
            if let Some(application) = signatures.abi_for_boundary(*symbol) {
                add(*application
                    .as_ref()
                    .map_err(|reason| unavailable(entry, reason.clone()))?);
                continue;
            }
            let surface = services
                .declaration(ProviderSchemaDeclaration::BoundaryTrait(*symbol))
                .ok_or_else(|| {
                    unavailable(entry, "metadata declaration has no exact boundary schema")
                })?;
            for requirement in surface.exact_requirements().map_err(|error| {
                unavailable(entry, format!("calling metadata requirements: {error:?}"))
            })? {
                let application = signatures.abi_for_boundary(requirement).ok_or_else(|| {
                    unavailable(
                        entry,
                        "boundary metadata requirement has no retained ABI application",
                    )
                })?;
                add(*application
                    .as_ref()
                    .map_err(|reason| unavailable(entry, reason.clone()))?);
            }
            for request in signatures
                .calling()
                .requests()
                .filter(|request| request.boundary == *symbol)
            {
                let instances = signatures.calling().instances(request).map_err(|errors| {
                    unavailable(entry, format!("requested calling metadata: {errors:?}"))
                })?;
                for instance in instances {
                    for applied in signatures.calling().signatures(instance) {
                        if let Some(reason) = &applied.abi_refusal {
                            return Err(unavailable(entry, reason.clone()));
                        }
                        if applied.abi_signatures.is_empty() {
                            return Err(unavailable(
                                entry,
                                "requested calling application has no exact ABI",
                            ));
                        }
                        for id in &applied.abi_signatures {
                            add(*id);
                        }
                    }
                }
            }
        }
        // A declaration ABI and a boundary's use of it are distinct rows.
        // Keep the association from the same exact source declaration and
        // catalog application that produced the requested ABI.
        let boundary_applications = self
            .boundaries
            .rows
            .iter()
            .map(
                |(boundary, source)| terminal_psi::CallingSignatureDeclaration {
                    boundary,
                    application: signatures
                        .abi_for_boundary(source.source)
                        .cloned()
                        .unwrap_or_else(|| {
                            Err("boundary declaration has no retained ABI application".into())
                        }),
                },
            )
            .collect();
        let outer = std::mem::take(&mut self.scope);
        let prepared = (|| {
            let mut declarations = Vec::with_capacity(ids.len());
            for id in ids {
                let mut matching = signatures
                    .abi_signatures()
                    .iter()
                    .filter(|row| row.id == id);
                let row = matching
                    .next()
                    .ok_or_else(|| unavailable(entry, "metadata ABI application is absent"))?;
                if matching.next().is_some() {
                    return Err(unavailable(
                        entry,
                        "metadata ABI application has competing declarations",
                    ));
                }
                let identity = CallingAbiIdentity {
                    requirement_identity: row.requirement_identity.clone(),
                    declaring_application: row.declaring_application.clone(),
                    operands: row.operands.clone(),
                };
                identity
                    .validate()
                    .map_err(|reason| unavailable(entry, reason))?;
                if identity.id() != row.id {
                    return Err(unavailable(
                        entry,
                        "metadata ABI coordinate differs from its exact identity",
                    ));
                }
                let selected = row
                    .projections_in_context(typed, self.purpose)
                    .next()
                    .ok_or_else(|| {
                        unavailable(
                            entry,
                            "calling ABI has no declaration in the selected checked context",
                        )
                    })?;
                let storage = self.calling_abi_projected_storage(entry, row);
                declarations.push(PreparedAbi {
                    identity,
                    formals: selected.formals.clone(),
                    default_eligible: selected.default_eligible,
                    physical_owner: selected.physical_owner.clone(),
                    storage,
                });
            }
            Ok(PreparedCallingAbis {
                entry,
                declarations,
                boundary_applications,
            })
        })();
        self.scope = outer;
        prepared
    }

    fn calling_abi_projected_storage(
        &mut self,
        entry: MachineId,
        row: &typed_trees::service_declarations::signatures::PreparedBoundaryAbiSignature,
    ) -> Result<PreparedStorage, String> {
        let typed = self.typed;
        let selected = row.projections_in_context(typed, self.purpose).next();
        let selected = selected
            .ok_or("calling ABI has no source projection in the selected checked context")?;
        let mut previous: Option<PreparedStorage> = None;
        for original in row.projections_in_context(typed, self.purpose) {
            if original.formals != selected.formals
                || original.default_eligible != selected.default_eligible
                || original.physical_owner != selected.physical_owner
            {
                return Err("calling ABI source copies disagree on their declaration in the selected checked context".into());
            }
            let telescope = original.telescope.as_ref().map_err(Clone::clone)?;
            if !telescope.binders.is_empty() || !telescope.direct_callbacks.is_empty() {
                return Err(
                    "calling ABI callback storage has no retained shared projection".into(),
                );
            }
            let projection = original.storage.as_ref().map_err(Clone::clone)?;
            if !projection.substitutions.is_empty() {
                return Err(
                    "calling ABI storage requires its retained source substitutions".into(),
                );
            }
            if projection.parameters.len() != telescope.parameters.len() {
                return Err("calling ABI source parameters differ from its telescope".into());
            }
            let parameters = projection
                .parameters
                .iter()
                .map(|reference| self.calling_abi_parameter(entry, *reference))
                .collect::<Result<Vec<_>, _>>()?;
            // An omitted source return annotation is Unit. Verify
            // that custody on this original declaration; a lost
            // projected type of an explicit result is not Unit.
            let omitted_unit = !projection.result.is_valid()
                && {
                    if let Some((_, signature)) = typed.trait_machine_signature(original.source) {
                        !signature.return_type.is_valid()
                    } else if let Some(machine) = typed.machine_by_symbol(original.source) {
                        matches!(typed.machine_states(machine), [state] if !state.return_type.is_valid())
                    } else {
                        false
                    }
                };
            if !omitted_unit
                && !typed
                    .type_reference_table
                    .contains_type_reference(projection.result)
            {
                return Err("calling ABI result lost its retained source type".into());
            }
            let result = if omitted_unit
                || matches!(
                    typed.type_reference_table.type_reference(projection.result),
                    TypeReferenceNode::Unit
                ) {
                None
            } else {
                Some(self.calling_abi_storage(entry, projection.result)?)
            };
            let storage = PreparedStorage {
                parameters,
                result,
                telescope: telescope.clone(),
            };
            if let Some(previous) = &previous {
                if previous.parameters != storage.parameters {
                    return Err("calling ABI source copies disagree on parameter storage in the selected checked context".into());
                }
                if previous.result != storage.result {
                    return Err("calling ABI source copies disagree on declared result storage in the selected checked context".into());
                }
                if previous.telescope != storage.telescope {
                    return Err("calling ABI source copies disagree on callback parameters in the selected checked context".into());
                }
            } else {
                previous = Some(storage);
            }
        }
        previous.ok_or_else(|| "calling ABI has no retained source storage projection".into())
    }

    fn calling_abi_storage(
        &mut self,
        entry: MachineId,
        reference: TypeReferenceHandle,
    ) -> Result<StructuralTypeId, String> {
        if !self
            .typed
            .type_reference_table
            .contains_type_reference(reference)
        {
            return Err("calling ABI storage lost its retained source type".into());
        }
        // The ABI key retains this occurrence's full qualified source type.
        // Only its outer carrier is described here: nested field constraints
        // still pass through the shared reader and may not be erased.
        let reference = self
            .storage_carrier(entry, reference)
            .map_err(|error| error.detail)?;
        let value = self
            .value_type(entry, reference)
            .map_err(|error| error.detail)?;
        self.storage_type(entry, reference, value)
            .map_err(|error| error.detail)
    }

    fn calling_abi_parameter(
        &mut self,
        entry: MachineId,
        reference: TypeReferenceHandle,
    ) -> Result<CallingAbiParameter, String> {
        if !self
            .typed
            .type_reference_table
            .contains_type_reference(reference)
        {
            return Err("calling ABI parameter lost its retained source type".into());
        }
        let (reference, access, referent) = match self
            .typed
            .type_reference_table
            .type_reference(reference)
        {
            TypeReferenceNode::Reference {
                referee,
                access,
                lifetime,
            } => {
                if lifetime.is_some() {
                    return Err(
                        "calling ABI reference storage requires its retained named lifetime".into(),
                    );
                }
                let access = match access {
                    language_core::ReferenceAccess::Shared => StructuralAccess::SharedBorrow,
                    language_core::ReferenceAccess::Mutable => StructuralAccess::MutableBorrow,
                    language_core::ReferenceAccess::WriteOnly => StructuralAccess::WriteOnlyBorrow,
                };
                let (carrier, referent) = self.borrowed_formal_carrier(reference, *referee);
                (carrier, access, referent)
            }
            _ => (
                reference,
                StructuralAccess::Owned,
                crate::body::BorrowedReferent::Value,
            ),
        };
        let structural_type = if referent == crate::body::BorrowedReferent::Slice
            && access == StructuralAccess::MutableBorrow
        {
            let value = self
                .formal_value_type(entry, reference, referent)
                .map_err(|error| error.detail)?;
            self.storage_type(entry, reference, value)
                .map_err(|error| error.detail)?
        } else {
            self.calling_abi_storage(entry, reference)?
        };
        // Callback closure walks the same direct carrier as the ABI formal,
        // including a borrowed slice's descriptor. A represented parameter
        // still needs its callback storage checked.
        let callback_backing = self.records.get(structural_type).map(|_| structural_type);
        Ok(CallingAbiParameter {
            structural_type,
            callback_backing,
            access,
        })
    }
}

impl PreparedCallingAbis {
    /// Publish while constructing the unaccepted module. This never registers
    /// a boundary operation or rewrites storage after acceptance/publication.
    pub(crate) fn publish(self, module: &mut TerminalModule) -> Checked<()> {
        let mut identities = std::mem::take(&mut module.identities).into_builder();
        let rows = self
            .declarations
            .into_iter()
            .map(|prepared| {
                let id = prepared.identity.id();
                let storage = prepared.storage.map(|storage| CallingAbiStorage {
                    parameters: storage.parameters,
                    result: storage.result,
                    telescope: intern_telescope(storage.telescope, &mut identities),
                });
                CallingAbiDeclaration {
                    id,
                    requirement_identity: identities
                        .intern(&prepared.identity.requirement_identity),
                    declaring_application: identities
                        .intern(&prepared.identity.declaring_application),
                    operands: prepared
                        .identity
                        .operands
                        .iter()
                        .map(|operand| CallingStaticOperand {
                            ordinal: operand.ordinal,
                            kind: operand.kind,
                            identity: identities.intern(&operand.identity),
                        })
                        .collect(),
                    formals: prepared.formals,
                    default_eligible: prepared.default_eligible,
                    physical_owner: prepared.physical_owner,
                    storage,
                }
            })
            .collect::<Vec<_>>();
        module.identities = identities.build();
        module.calling_abi_signatures = crate::calling_publication::join_abi_declarations(
            self.entry,
            &module.identities,
            &module.calling_abi_signatures,
            &rows,
        )?;
        for application in self.boundary_applications {
            if module.boundary_machine(application.boundary).is_none() {
                return Err(unavailable(
                    self.entry,
                    "calling ABI association lost its exact boundary",
                ));
            }
            if let Some(existing) = module
                .calling_signatures
                .iter()
                .find(|row| row.boundary == application.boundary)
            {
                if existing != &application {
                    return Err(unavailable(
                        self.entry,
                        "boundary calling metadata conflicts with its exact ABI application",
                    ));
                }
            } else {
                module.calling_signatures.push(application);
            }
        }
        module.calling_signatures.sort_by_key(|row| row.boundary);
        Ok(())
    }
}

fn intern_telescope(
    source: BoundaryTelescope,
    identities: &mut TerminalIdentityTableBuilder,
) -> CallingSignatureTelescope {
    CallingSignatureTelescope {
        parameters: source
            .parameters
            .into_iter()
            .map(|formal| CallingRuntimeFormal {
                identity: formal.identity,
                native_ordinal: formal.native_ordinal,
            })
            .collect(),
        binders: source
            .binders
            .into_iter()
            .map(|binder| CallingCallbackBinder {
                binder: binder.binder,
                requirement: binder.requirement,
                requirement_identity: identities.intern(&binder.requirement_identity),
                static_machine_ordinal: binder.static_machine_ordinal,
                invokes_synchronously: binder.invokes_synchronously,
            })
            .collect(),
        direct_callbacks: source
            .direct_callbacks
            .into_iter()
            .map(|callback| CallingDirectCallback {
                identity: callback.identity,
                native_ordinal: callback.native_ordinal,
                binder: callback.binder,
                requirement: callback.requirement,
            })
            .collect(),
    }
}

#[cfg(test)]
mod descriptor_storage_tests {
    use super::*;
    use language_semantics::Multiplicity;
    use semantic_vocabulary::{IdTable, IntegerType, ScalarType};
    use symbols::{
        BuiltinTypeAtom, SymbolKind, SymbolNameRef, SymbolTableBuilder, builtin_type_symbols,
    };
    use typed_trees::data::{DataDefinition, DataField, DataMember, DataProperties};
    use typed_trees::types::{DomainConstraint, TypeConstraintNode};

    fn fixture() -> (
        typed_trees::TypedTrees,
        TypeReferenceHandle,
        TypeReferenceHandle,
        TypeReferenceHandle,
        TypeReferenceHandle,
    ) {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = builder.insert_children(
            root,
            builtin_type_symbols().into_iter().chain([
                (SymbolKind::Data, SymbolNameRef::Static("BufferClaim")),
                (SymbolKind::Domain, SymbolNameRef::Static("Granted")),
                (SymbolKind::Data, SymbolNameRef::Static("Envelope")),
            ]),
        );
        let declaration = children.handles().nth(symbols::BUILTIN_TYPE_COUNT).unwrap();
        let domain = children
            .handles()
            .nth(symbols::BUILTIN_TYPE_COUNT + 1)
            .unwrap();
        let envelope_symbol = children
            .handles()
            .nth(symbols::BUILTIN_TYPE_COUNT + 2)
            .unwrap();
        let inner = builder
            .insert_children(
                envelope_symbol,
                [(SymbolKind::Field, SymbolNameRef::Static("inner"))],
            )
            .handle_at(0);
        let fields = builder.insert_children(
            declaration,
            [
                (SymbolKind::Field, SymbolNameRef::Static("origin")),
                (SymbolKind::Field, SymbolNameRef::Static("count")),
            ],
        );
        let mut typed = typed_trees::TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let mut named = |symbol, name: &str| {
            typed.type_reference_table.insert(TypeReferenceNode::Named {
                symbol,
                name: name.into(),
            })
        };
        let address = named(
            children
                .handles()
                .nth(BuiltinTypeAtom::Address.ordinal())
                .unwrap(),
            "addr",
        );
        let length = named(
            children
                .handles()
                .nth(BuiltinTypeAtom::U64.ordinal())
                .unwrap(),
            "u64",
        );
        let record = named(declaration, "BufferClaim");
        let envelope = named(envelope_symbol, "Envelope");
        let members = typed.data_members.insert_many([
            DataMember::Field(DataField {
                symbol: fields.handle_at(0),
                name: "origin".into(),
                type_reference: address,
                ..Default::default()
            }),
            DataMember::Field(DataField {
                symbol: fields.handle_at(1),
                name: "count".into(),
                type_reference: length,
                ..Default::default()
            }),
        ]);
        typed.push_data_definition(DataDefinition {
            symbol: declaration,
            name: "BufferClaim".into(),
            members,
            properties: DataProperties {
                multiplicity: Multiplicity::Linear,
                carry: None,
            },
            ..Default::default()
        });
        let members = typed
            .data_members
            .insert_many([DataMember::Field(DataField {
                symbol: inner,
                name: "inner".into(),
                type_reference: record,
                ..Default::default()
            })]);
        typed.push_data_definition(DataDefinition {
            symbol: envelope_symbol,
            name: "Envelope".into(),
            members,
            properties: DataProperties {
                multiplicity: Multiplicity::Affine,
                carry: None,
            },
            ..Default::default()
        });
        let constraints =
            typed
                .type_reference_table
                .insert_constraints([TypeConstraintNode::Domain(DomainConstraint {
                    name: "Granted".into(),
                    symbol: domain,
                    ..Default::default()
                })]);
        let qualified = typed
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type: record,
                constraints,
            });
        (typed, address, record, qualified, envelope)
    }

    #[test]
    fn borrowed_slice_abi_formals_use_the_checked_descriptor() {
        use language_core::ReferenceAccess;

        let (mut typed, address, _, _, _) = fixture();
        let byte_symbol = typed
            .symbols
            .builtin_atom_symbol(BuiltinTypeAtom::U8)
            .unwrap();
        let byte = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: byte_symbol,
            name: "u8".into(),
        });
        let bytes = typed
            .type_reference_table
            .insert(TypeReferenceNode::Slice { element_type: byte });
        let addresses = typed.type_reference_table.insert(TypeReferenceNode::Slice {
            element_type: address,
        });
        let cases = [
            (
                bytes,
                ReferenceAccess::Shared,
                StructuralAccess::SharedBorrow,
            ),
            (
                bytes,
                ReferenceAccess::Mutable,
                StructuralAccess::MutableBorrow,
            ),
            (
                addresses,
                ReferenceAccess::Shared,
                StructuralAccess::SharedBorrow,
            ),
        ];
        let written = cases.map(|(referee, access, expected_access)| {
            let reference = typed
                .type_reference_table
                .insert(TypeReferenceNode::Reference {
                    referee,
                    access,
                    lifetime: None,
                });
            (reference, expected_access)
        });
        let stored_mutable = typed
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: written[1].0,
                access: ReferenceAccess::Mutable,
                lifetime: None,
            });
        let entry = MachineId::new(1).unwrap();
        let mut program = Program::new(&typed, false);
        let checked = written.map(|(reference, access)| {
            let ordinary = program.value_type(entry, reference).unwrap();
            let formal = program
                .formal_value_type(entry, reference, crate::body::BorrowedReferent::Slice)
                .unwrap();
            let crate::ValueType::Record(carrier, _, _) = formal else {
                panic!("a borrowed slice has structural descriptor storage");
            };
            if access == StructuralAccess::MutableBorrow {
                assert_ne!(formal, ordinary);
                assert_eq!(
                    program.data_type(entry, carrier).unwrap().shape,
                    crate::StorageShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView { access: None }
                    )
                );
            } else {
                assert_eq!(formal, ordinary);
            }
            carrier
        });
        let crate::ValueType::Record(stored_carrier, _, _) =
            program.value_type(entry, written[1].0).unwrap()
        else {
            panic!("a stored view has structural descriptor storage");
        };
        program.begin_declaration_storage(entry, 0).unwrap();
        let owned = program.calling_abi_parameter(entry, bytes).unwrap();
        assert_eq!(owned.access, StructuralAccess::Owned);
        for ((reference, access), carrier) in written.into_iter().zip(checked) {
            assert_ne!(carrier, owned.structural_type);
            assert_eq!(
                program.calling_abi_parameter(entry, reference).unwrap(),
                CallingAbiParameter {
                    structural_type: carrier,
                    callback_backing: Some(carrier),
                    access,
                },
            );
        }
        assert_eq!(
            program
                .calling_abi_parameter(entry, stored_mutable)
                .unwrap(),
            CallingAbiParameter {
                structural_type: stored_carrier,
                callback_backing: Some(stored_carrier),
                access: StructuralAccess::MutableBorrow,
            },
        );
    }

    #[test]
    fn calling_parameters_retain_their_complete_direct_callback_backing() {
        let (mut typed, address, record, qualified, _) = fixture();
        let borrowed = typed
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: qualified,
                access: language_core::ReferenceAccess::Shared,
                lifetime: None,
            });
        let constrained_nested = typed
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: borrowed,
                access: language_core::ReferenceAccess::Shared,
                lifetime: None,
            });
        let stored_reference = typed
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: record,
                access: language_core::ReferenceAccess::Shared,
                lifetime: None,
            });
        let nested = typed
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: stored_reference,
                access: language_core::ReferenceAccess::Shared,
                lifetime: None,
            });
        let entry = MachineId::new(1).unwrap();
        let mut program = Program::new(&typed, false);
        program.begin_declaration_storage(entry, 0).unwrap();
        let owned = program.calling_abi_parameter(entry, record).unwrap();
        let loan = program.calling_abi_parameter(entry, borrowed).unwrap();
        assert_eq!(owned.access, StructuralAccess::Owned);
        assert_eq!(loan.access, StructuralAccess::SharedBorrow);
        assert_eq!(loan.structural_type, owned.structural_type);
        for parameter in [&owned, &loan] {
            assert_eq!(parameter.callback_backing, Some(parameter.structural_type));
            assert!(program.records.get(parameter.structural_type).is_some());
        }
        let scalar = program.calling_abi_parameter(entry, address).unwrap();
        assert_eq!(scalar.callback_backing, Some(scalar.structural_type));
        // Only the ordinary outer loan shell is removed. A stored inner
        // reference still owes its own referent's domain declaration.
        assert_eq!(
            program
                .calling_abi_parameter(entry, constrained_nested)
                .unwrap_err(),
            "type domain application without its prepared theory",
        );
        let descriptor = program.calling_abi_parameter(entry, nested).unwrap();
        assert_eq!(
            descriptor.callback_backing,
            Some(descriptor.structural_type)
        );
        assert_ne!(descriptor.structural_type, owned.structural_type);
        assert!(
            matches!(program.records.get(descriptor.structural_type).unwrap().shape,
            crate::body::StorageShape::Reference { referent, access: StructuralAccess::SharedBorrow }
                if referent == owned.structural_type)
        );
    }

    #[test]
    fn actual_linear_address_carrier_is_described_without_admitting_a_body_or_grant() {
        let (typed, address, record, qualified, envelope) = fixture();
        let entry = MachineId::new(1).unwrap();
        let original = typed.type_reference_table.type_reference(qualified).clone();
        let mut executable = Program::new(&typed, false);
        // An address is a plain scalar in a body, carried and compared
        // (`atom_type`). The record that holds its custody is still not
        // admitted there.
        assert_eq!(
            executable.value_type(entry, address).unwrap().scalar(),
            Some(ScalarType::Integer(
                IntegerType::supported_address_representation()
            ))
        );
        assert_eq!(
            executable.value_type(entry, record).unwrap_err().detail,
            "type data with custody"
        );
        let mut program = Program::new(&typed, false);
        program.begin_declaration_storage(entry, 0).unwrap();
        let storage = program.calling_abi_storage(entry, qualified).unwrap();
        assert!(program.records.get(storage).unwrap().owns_linear_claim);
        assert!(!program.records.get(storage).unwrap().copyable);
        assert_eq!(
            typed.type_reference_table.type_reference(qualified),
            &original
        );
        let nested = program.calling_abi_storage(entry, envelope).unwrap();
        assert!(program.records.get(nested).unwrap().owns_linear_claim);
        // ABI metadata must name the represented backing even when the
        // parameter itself owns a claim or is an ordinary scalar.
        let parameters = [address, qualified, envelope]
            .map(|reference| program.calling_abi_parameter(entry, reference).unwrap());
        let fields = program.records.get(storage).unwrap().fields();
        assert_eq!(fields.len(), 2);
        assert_eq!(
            fields[0].value_type.scalar(),
            Some(ScalarType::Integer(
                IntegerType::supported_address_representation()
            ))
        );
        let bodies = IdTable::default();

        let declared = crate::lower::places::declare(
            entry,
            &program.records,
            &bodies,
            std::iter::empty::<&crate::Signature>(),
            &program.byte_domains,
            Some(program.typed),
            TerminalIdentityTableBuilder::default(),
        )
        .unwrap();
        for parameter in parameters {
            assert_eq!(parameter.callback_backing, Some(parameter.structural_type));
            assert!(
                declared
                    .structural_types
                    .iter()
                    .any(|row| Some(row.id) == parameter.callback_backing)
            );
        }
        let canonical = declared
            .structural_types
            .iter()
            .find(|row| row.id == storage)
            .unwrap();
        assert!(canonical.owns_linear_claim);
        assert!(!canonical.copyable);
        assert!(!canonical.no_code_affine_cleanup);
        assert!(!canonical.descriptor_cleanup);
        assert!(program.source_record_correspondence(entry).is_ok());
        assert!(
            program
                .body(entry)
                .unwrap_err()
                .detail
                .contains("after storage metadata")
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use source::{
        DependencyScope, SourceMap, SourceOrigin, SourceResolutionStratum, SourceSpan, Span,
    };
    use symbols::{BuiltinTypeAtom, SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::service_declarations::signatures::{
        PreparedAbiSource, PreparedAbiStorage, PreparedBoundaryAbiSignature,
    };

    fn projection(source: SymbolHandle, result: TypeReferenceHandle) -> PreparedAbiSource {
        PreparedAbiSource {
            source,
            formals: Vec::new(),
            default_eligible: true,
            physical_owner: Ok(semantic_vocabulary::CallingDeclarationOwner::BoundaryTrait),
            telescope: Ok(BoundaryTelescope::default()),
            storage: Ok(PreparedAbiStorage {
                parameters: Vec::new(),
                result,
                substitutions: Vec::new(),
            }),
        }
    }

    fn fixture() -> (
        typed_trees::TypedTrees,
        PreparedBoundaryAbiSignature,
        TypeReferenceHandle,
    ) {
        let mut sources = SourceMap::default();
        let instances = [DependencyScope::Product, DependencyScope::Build].map(|scope| {
            sources
                .add_checked_instance(
                    "package/api.omg".into(),
                    "read",
                    "package".into(),
                    None,
                    SourceOrigin::User,
                    SourceResolutionStratum::Base,
                    scope,
                )
                .source_id
        });
        let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = SymbolTableBuilder::child_handles(
            builder.insert_children(
                root,
                symbols::builtin_type_symbols()
                    .into_iter()
                    .chain(instances.map(|instance| {
                        (
                            SymbolKind::Machine,
                            SymbolNameRef::Source(SourceSpan::new(instance, Span::new(0, 4))),
                        )
                    })),
            ),
        )
        .collect::<Vec<_>>();
        let owners = &children[children.len() - 2..];
        let mut typed = typed_trees::TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let types = [
            (
                BuiltinTypeAtom::Integer(numerics::integer_carrier::FixedInteger::unsigned(32)),
                "u32",
            ),
            (BuiltinTypeAtom::U64, "u64"),
        ]
        .map(|(atom, name)| {
            typed.type_reference_table.insert(TypeReferenceNode::Named {
                symbol: children[atom.ordinal()],
                name: name.into(),
            })
        });
        let row = PreparedBoundaryAbiSignature {
            id: CallingSignatureId::for_application(
                "Service::read",
                "Service",
                std::iter::empty::<(u32, semantic_vocabulary::CallingOperandKind, &str)>(),
            ),
            requirement_identity: "Service::read".into(),
            declaring_application: "Service".into(),
            operands: Vec::new(),
            projections: vec![
                projection(owners[0], types[0]),
                projection(owners[1], types[1]),
            ],
        };
        (typed, row, types[1])
    }

    #[test]
    fn calling_storage_uses_its_checked_context() {
        let (typed, row, _) = fixture();
        for (purpose, width) in [(DependencyScope::Product, 32), (DependencyScope::Build, 64)] {
            let mut program = Program::new(&typed, false);
            program.purpose = purpose;
            let entry = MachineId::new(1).unwrap();
            program.begin_declaration_storage(entry, 0).unwrap();
            let storage = program.calling_abi_projected_storage(entry, &row).unwrap();
            let crate::StorageShape::PrimitiveScalar {
                scalar_type: semantic_vocabulary::ScalarType::Integer(integer),
                ..
            } = program.records.get(storage.result.unwrap()).unwrap().shape
            else {
                panic!("integer result")
            };
            assert_eq!(integer.bits(), Some(width));
        }
    }

    #[test]
    fn unavailable_calling_projection_stays_with_its_source_context() {
        let (typed, mut row, _) = fixture();
        row.projections[1].telescope = Err("unavailable Build callback telescope".into());
        row.projections[1].storage = Err("unavailable Build storage".into());
        let mut program = Program::new(&typed, false);
        let entry = MachineId::new(1).unwrap();
        program.begin_declaration_storage(entry, 0).unwrap();
        assert!(program.calling_abi_projected_storage(entry, &row).is_ok());
        program.purpose = DependencyScope::Build;
        assert_eq!(
            program
                .calling_abi_projected_storage(entry, &row)
                .err()
                .unwrap(),
            "unavailable Build callback telescope"
        );
        row.projections[1].telescope = Ok(BoundaryTelescope::default());
        assert_eq!(
            program
                .calling_abi_projected_storage(entry, &row)
                .err()
                .unwrap(),
            "unavailable Build storage"
        );
        row.projections.remove(1);
        assert!(
            program
                .calling_abi_projected_storage(entry, &row)
                .err()
                .unwrap()
                .contains("no source projection in the selected checked context")
        );
    }

    #[test]
    fn conflicting_calling_storage_in_one_context_still_refuses() {
        let (typed, mut row, other_result) = fixture();
        let source = row.projections[0].source;
        row.projections.push(projection(source, other_result));
        let mut program = Program::new(&typed, false);
        let entry = MachineId::new(1).unwrap();
        program.begin_declaration_storage(entry, 0).unwrap();
        assert_eq!(
            program
                .calling_abi_projected_storage(entry, &row)
                .err()
                .unwrap(),
            "calling ABI source copies disagree on declared result storage in the selected checked context"
        );
    }
}
