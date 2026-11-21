use crate::declarations::{data, domains, machine::machine_header};
use crate::declarations::regions::{self, slot_carrier};
use crate::lowering::types;
use crate::machines::{body, branches, calls};
use crate::{DataId, LoweredMachine, LoweredPackage, LoweringError, LoweringTypes, MachineId, ScalarResult, TypeShape, site_at};
use omega_prepared::PreparedTrees;
use omega_syntax::NodeKind;
use omega_typed::ValueType;
use std::collections::HashMap;
use std::num::NonZeroU64;

/// Lower each local executable declaration once, preserving the order of its
/// operands. Declaration identities determine machine order independently of
/// the order in which the caller supplied the source files.
pub fn lower_package(prepared: &PreparedTrees<'_>) -> Result<LoweredPackage, LoweringError> {
    let typed = prepared.typed();
    let trees: Vec<_> = typed.trees().collect();
    let signature_by_declaration: HashMap<_, _> = typed
        .signatures
        .iter()
        .map(|sig| (&sig.machine, sig.as_ref()))
        .collect();
    let math_interfaces: HashMap<_, _> = typed
        .mathematical_interfaces
        .iter()
        .map(|(id, signature, deps)| (id, (signature, deps.as_slice())))
        .collect();
    let mut mathematics = Vec::new();
    let mut theorems = Vec::new();
    let mut data_templates = Vec::new();
    let mut conformances = Vec::new();
    let mut signatures = Vec::new();
    let library_trees: Vec<_> = typed
        .library
        .filter(|_| !typed.external_completions.is_empty())
        .into_iter()
        .flat_map(|library| library.trees())
        .collect();
    let mut data_schemas = Vec::new();
    let mut aliases = Vec::new();
    let mut domain_schemas = Vec::new();
    let mut domain_templates = Vec::new();
    let mut trait_schemas = Vec::new();
    for tree in &trees {
        let file = tree.resolved().file;
        let mut declarations = file.nodes[0].children.iter().rev().copied().collect::<Vec<_>>();
        while let Some(child) = declarations.pop() {
            let kind = file.nodes[child].kind;
            if matches!(kind, NodeKind::Machine | NodeKind::AsmMachine) {
                let id = tree
                    .resolved()
                    .declaration_at(child)
                    .ok_or_else(|| LoweringError::new(&site_at(tree, child), "machine has no resolved declaration"))?;
                let sig = signature_by_declaration
                    .get(id)
                    .copied()
                    .filter(|sig| sig.node == file.node_id(child))
                    .ok_or_else(|| LoweringError::new(&site_at(tree, child), "machine has no local typed signature"))?;
                // A theorem with logical hypotheses publishes no executable machine.
                if file.child(child, NodeKind::Hypothesis).is_some() {
                    theorems.push((sig, tree, crate::declarations::hypotheses::header(prepared, tree, sig)?));
                    continue;
                }
                signatures.push(sig);
                if let Some(body) = file.child(child, NodeKind::Body) {
                    declarations.extend(
                        file.nodes[body]
                            .children
                            .iter()
                            .rev()
                            .copied()
                            .filter(|&n| file.nodes[n].kind == NodeKind::Machine),
                    );
                }
            } else if kind == NodeKind::MathLet {
                mathematics.push(crate::declarations::mathematics::lower(typed, tree, child, &math_interfaces)?);
            } else if kind == NodeKind::Data {
                if let Some(template) = crate::declarations::data_templates::read(prepared, tree, child, &mut declarations)? {
                    data_templates.push(template);
                    continue;
                }
                match data::read_data(prepared, tree, child, &mut declarations)? {
                    data::DataDeclaration::Schema(schema) => data_schemas.push(schema),
                    data::DataDeclaration::Alias(alias) => aliases.push(alias),
                }
            } else if kind == NodeKind::Conformance {
                conformances.push(crate::declarations::members::conformance(typed, tree, child, &mut declarations)?);
            } else if kind == NodeKind::Trait {
                trait_schemas.push(crate::declarations::traits::read_trait(prepared, tree, child, &signature_by_declaration)?);
            } else if kind == NodeKind::Domain {
                let schema = domains::read_domain(prepared, tree, child)?;
                if matches!(schema.carrier, ValueType::Parameter { .. }) {
                    domain_templates.push(schema.into_template()?);
                } else {
                    domain_schemas.push(schema);
                }
            } else if !matches!(kind, NodeKind::Const | NodeKind::Use) {
                return Err(LoweringError::new(&site_at(tree, child), format!("unsupported Psi declaration: {kind:?}")));
            }
        }
    }
    for constant in &typed.constants {
        if prepared.open_target_constant(&constant.declaration).is_some() {
            return Err(LoweringError::new(
                &constant.initializer.expression,
                "constant requires a closed target before lowering",
            ));
        }
    }
    // A terminal call has no normal successor, but its signature still
    // publishes an ordinary data type with its defining library predicate.
    let required: std::collections::BTreeSet<_> = typed
        .external_completions
        .iter()
        .filter_map(|completion| signature_by_declaration.get(&completion.requirement))
        .filter_map(|sig| match &sig.result {
            ValueType::Nominal(id) => Some(id.as_ref()),
            _ => None,
        })
        .collect();
    for tree in &library_trees {
        for (node, id) in tree.resolved().declaration_nodes() {
            if id.kind != NodeKind::Data || !required.contains(id) {
                continue;
            }
            let mut nested = Vec::new();
            let data::DataDeclaration::Schema(schema) = data::read_data(prepared, tree, node, &mut nested)? else {
                return Err(LoweringError::new(&site_at(tree, node), "imported nominal type has no data declaration"));
            };
            if !nested.is_empty() {
                return Err(LoweringError::new(&schema.site, "imported data requires its nested declarations"));
            }
            data_schemas.push(schema);
        }
    }
    theorems.sort_unstable_by(|(_, _, a), (_, _, b)| a.declaration.cmp(&b.declaration));
    // All theorem headers are available before any body can cite a theorem.
    let theorem_headers = theorems.iter().map(|(sig, _, theorem)| {
        (&sig.machine, crate::machines::theorem_citations::TheoremCallee { signature: sig, theorem })
    }).collect();
    let proofs = theorems.iter().map(|(sig, tree, _)| {
        crate::declarations::hypotheses::proof_steps(prepared, tree, sig, &theorem_headers)
    }).collect::<Result<Vec<_>, _>>()?;
    let (theorem_signatures, theorems): (Vec<_>, Vec<_>) = theorems.into_iter().zip(proofs).map(|((sig, _, mut theorem), proof)| {
        theorem.proof = proof;
        (sig, theorem)
    }).unzip();
    signatures.sort_unstable_by(|a, b| a.machine.cmp(&b.machine));
    let instances = super::applications::plan(prepared, &signatures, &trees, typed)?;
    let members = crate::declarations::members::plan(typed, &trees, &instances, &signature_by_declaration)?;
    let machine_templates = crate::declarations::machine_templates::plan(prepared, &trees, &signatures, &instances, &trait_schemas)?;
    let effects: HashMap<_, _> = typed
        .machine_effects
        .iter()
        .map(|e| (&e.machine, e.as_ref()))
        .collect();
    let stored_ceilings = super::callbacks::stored_ceilings(typed);
    let mut headers = Vec::with_capacity(instances.len());
    let mut carriers = Vec::new();
    data_schemas.sort_unstable_by(|a, b| a.declaration.cmp(b.declaration));
    let applications = super::applied_data::collect(typed, &data_templates);
    for application in &applications {
        application.collect_carriers(&mut carriers);
    }
    members.collect_carriers(&mut carriers);
    for alias in &aliases {
        if let data::PendingAlias::Structural { carrier, site, .. } = alias {
            carriers.push((*carrier, site.clone()));
        }
    }
    trait_schemas.sort_unstable_by(|a, b| a.declaration.cmp(b.declaration));
    for schema in &trait_schemas {
        schema.collect_carriers(&mut carriers);
    }
    domain_templates.sort_unstable_by(|a, b| a.declaration.cmp(&b.declaration));
    domain_schemas.sort_unstable_by(|a, b| a.declaration.cmp(b.declaration));
    for domain in &domain_schemas {
        carriers.push((domain.carrier, domain.site.clone()));
    }
    let case_owners: std::collections::HashSet<_> = typed.case_owners.values().collect();
    for data in &data_schemas {
        // Normal cleanup can test an intact sum's discriminator even when
        // the machine has no authored Boolean expression.
        if case_owners.contains(data.declaration) {
            carriers.push((&ValueType::Bool, data.site.clone()));
        }
        carriers.push((&data.carrier, data.site.clone()));
        carriers.extend(data.value_indices.iter().map(|ty| (*ty, data.site.clone())));
        for field in data.fields() {
            carriers.push((data.field_carrier(field)?.0, field.site.clone()));
        }
    }
    let retained = instances.iter().flat_map(|i| i.tree.guarded_rows.iter().copied()).collect();
    for instance in &instances {
        let sig = &instance.signature;
        let tree = &instance.tree;
        for argument in &instance.application.arguments {
            // A static integer publishes its scalar carrier in the argument itself, not as a runtime type.
            if !matches!(argument, ValueType::MachineSelection(_) | ValueType::Implementation(_) | ValueType::StaticInteger { .. }) {
                carriers.push((argument, sig.site.clone()));
            }
        }
        let effect = effects
            .get(&sig.machine)
            .ok_or_else(|| LoweringError::new(&sig.site, "machine has no normalized effect row"))?;
        let realized_pause = super::callbacks::realized_pause(effect, &instance.callbacks, &signature_by_declaration, &effects, &stored_ceilings);
        headers.push(machine_header(prepared, instance, effect, realized_pause, &domain_schemas)?);
        crate::declarations::crash_routes::check_invocations(prepared, sig, &retained)?;
        let file = tree.resolved().file;
        let mut pending: Vec<_> = if sig.machine.kind == NodeKind::AsmMachine {
            vec![sig.node.node]
        } else {
            file.child(sig.node.node, NodeKind::Body).into_iter().collect()
        };
        while let Some(node) = pending.pop() {
            if file.nodes[node].kind == NodeKind::Machine {
                continue;
            }
            pending.extend(&file.nodes[node].children);
            if let Some(region) = tree.assembly_region(node) {
                carriers.extend(
                    region
                        .bindings
                        .iter()
                        .map(|(_, _, location)| (&location.ty, site_at(tree, node))),
                );
            }
            for record in tree.semantics_at(node) {
                if let omega_typed::NodeSemantics::Instruction(i) = record {
                    carriers.extend(i.operands.iter().filter_map(|o| match o {
                        omega_typed::TypedInstructionOperand::Value { ty, .. } | omega_typed::TypedInstructionOperand::Register { ty, .. } => {
                            Some((ty, i.site.clone()))
                        }
                        _ => None,
                    }));
                    carriers.extend(
                        i.implicit_locations
                            .iter()
                            .map(|(_, location)| (&location.ty, i.site.clone())),
                    );
                    if matches!(i.semantics.exit, crate::InstructionExit::Branch { .. }) {
                        carriers.push((&ValueType::Bool, i.site.clone()));
                    }
                }
            }
            // Guards and case-discriminator tests can need a Boolean with no source expression row.
            if matches!(file.nodes[node].kind, NodeKind::Match | NodeKind::Transition)
                && file.nodes[node].children.len() > 1
                && branches::has_guards(file, node)
                || typed.complete_case_patterns.contains_key(&file.node_id(node))
            {
                carriers.push((&ValueType::Bool, site_at(tree, node)));
            }
            for record in tree.semantics_at(node) {
                let (carrier, site) = match record {
                    // A static callable argument selects a machine; it is not a runtime descriptor.
                    omega_typed::NodeSemantics::Expression(e) if e.static_argument_slot.is_some() && matches!(e.ty, ValueType::Callable(_)) => continue,
                    omega_typed::NodeSemantics::Expression(e) => (&e.ty, &e.site),
                    omega_typed::NodeSemantics::Landing(e) => (&e.ty, &e.site),
                    _ => continue,
                };
                if types::place_reference(carrier).is_some()
                    || types::supports_value_copy(carrier)
                    || types::local_domain_members(carrier, &domain_schemas).is_some()
                    || matches!(carrier, ValueType::Callable(callable) if callable.closed_runtime)
                {
                    carriers.push((carrier, site.clone()));
                }
            }
        }
        for (span, carrier) in &sig.parameters {
            let mut site = sig.site.clone();
            site.span = *span;
            carriers.push((slot_carrier(carrier, &sig.machine), site));
        }
        if sig.result != ValueType::NoResult {
            let site = tree
                .resolved()
                .file
                .child(sig.node.node, NodeKind::Result)
                .map_or_else(|| sig.site.clone(), |node| site_at(tree, node));
            carriers.push((slot_carrier(&sig.result, &sig.machine), site));
        }
    }
    let mut types = LoweringTypes::from_declarations(
        carriers.iter().map(|(carrier, _)| *carrier),
        data_schemas
            .iter()
            .map(|data| data.declaration.clone())
            .collect(),
        applications.iter().map(|a| a.carrier.clone()).collect(),
        &domain_schemas,
    )
    .map_err(|(index, error)| LoweringError::new(&carriers[index].1, error.to_string()))?;
    let mut data = data_schemas
        .into_iter()
        .enumerate()
        .map(|(index, schema)| schema.lower(DataId::from_index(index), &types))
        .collect::<Result<Vec<_>, _>>()?;
    for application in applications {
        let id = DataId::from_index(data.len());
        data.push(application.lower(id, &types)?);
    }
    let mut aliases = aliases
        .into_iter()
        .map(|alias| alias.lower(&types))
        .collect::<Result<Vec<_>, _>>()?;
    aliases.sort_unstable_by(|a, b| a.declaration.cmp(&b.declaration));
    let traits = trait_schemas
        .into_iter()
        .map(|t| t.lower(&types, &domain_schemas))
        .collect::<Result<Vec<_>, _>>()?;
    for alias in &traits {
        if let Some(parent) = &alias.parent
            && !traits.iter().any(|base| base.declaration == parent.declaration)
        {
            return Err(LoweringError::new(&alias.site, "inheriting trait needs its packaged parent requirements"));
        }
        if let Some(restriction) = &alias.restriction
            && !traits.iter().any(|base| base.declaration == restriction.base.declaration)
        {
            return Err(LoweringError::new(&alias.site, "restricted trait needs its packaged base requirements"));
        }
    }
    // A runtime call owes each published `requires` clause of its contract, so
    // the contracts are lowered before any body that calls through them.
    let callable_contracts = types
        .callable_contracts()
        .iter()
        .map(|callable| crate::declarations::callables::contract(prepared, callable))
        .collect::<Result<Vec<_>, _>>()?;
    let mut machines = Vec::with_capacity(instances.len());
    let mut crossing_owners = Vec::with_capacity(instances.len());
    let mut callees = calls::Callees {
        runtime_pause: callable_contracts.iter().map(|contract| contract.pause).collect(),
        runtime_requirements: callable_contracts
            .iter()
            .map(|contract| contract.requires.len())
            .collect(),
        machine_pause: headers
            .iter()
            .map(|(contract, _)| contract.realized_pause)
            .collect(),
        theorems: theorem_signatures
            .iter()
            .zip(&theorems)
            .map(|(signature, theorem)| (&signature.machine, crate::machines::theorem_citations::TheoremCallee { signature, theorem }))
            .collect(),
        ..Default::default()
    };
    for (index, instance) in instances.iter().enumerate() {
        let sig = &*instance.signature;
        let ordinal = u64::try_from(index)
            .ok()
            .and_then(|n| n.checked_add(1))
            .and_then(NonZeroU64::new)
            .ok_or_else(|| LoweringError::new(&sig.site, "too many Psi machines"))?;
        let mut restricted_arguments = Vec::new();
        for (index, argument) in instance.application.arguments.iter().enumerate() {
            if let ValueType::Implementation(selected) = argument {
                let bound = crate::declarations::members::evidence_bound(typed, instance, index)?;
                // An exported wrapper also publishes its open template beside
                // this instance; see `machine_templates::plan`.
                if typed.refinement_bases.contains_key(&bound.declaration) {
                    let alias_id = crate::identity::declaration(&bound.declaration, &sig.site)?;
                    let selected_id = crate::identity::declaration(selected, &sig.site)?;
                    let alias = traits.iter().find(|t| t.declaration == alias_id).and_then(|t| t.restriction.as_ref());
                    let conformance = conformances.iter().find(|c| c.declaration == selected_id);
                    if !alias.zip(conformance).is_some_and(|(alias, c)| alias.base == c.implemented) {
                        return Err(LoweringError::new(&sig.site, "restricted evidence requires its packaged base conformance"));
                    }
                    restricted_arguments.push((*bound, selected.as_ref().clone()));
                }
            }
        }
        callees.local.entry(&sig.machine).or_default().insert(
            instance.application.arguments.as_slice(),
            calls::Callee {
                machine: MachineId(ordinal),
                callbacks: &instance.callbacks,
                restricted_arguments,
                signature: sig,
                requirement_count: headers[index].0.requires.len(),
            },
        );
    }
    let member_calls = members.lower(&types, &traits, &mut callees)?;
    for selection in typed.cleanups.iter().filter(|s| s.owner.kind == NodeKind::Data || domain_schemas.iter().any(|d| d.declaration == &s.owner)) {
        let declaration = crate::identity::declaration(&selection.owner, &selection.site)?;
        let target = callees
            .local
            .get(&selection.machine)
            .and_then(|targets| targets.get(selection.arguments.as_slice()))
            .ok_or_else(|| LoweringError::new(&selection.site, "nominal cleanup needs its closed selected body"))?;
        if callees.realized_pause(target.machine) != Default::default() {
            return Err(LoweringError::new(
                &selection.site,
                "implicit cleanup calls need a suspension position before their target may park",
            ));
        }
        let [(_, subject)] = target.signature.parameters.as_slice() else {
            return Err(LoweringError::new(&selection.site, "selected cleanup requires one owned subject"));
        };
        let ty = types
            .id_for(subject)
            .map_err(|e| LoweringError::new(&selection.site, e.to_string()))?;
        if selection.owner.kind == NodeKind::Domain {
            let domain = domain_schemas.iter().find(|d| d.declaration == &selection.owner).expect("selected domain in this package");
            if domain.carrier != subject || !matches!(subject, ValueType::Nominal(_)) || target.signature.result != ValueType::NoResult {
                return Err(LoweringError::new(&selection.site, "domain cleanup requires its closed nominal subject and unit result"));
            }
            types.register_domain_cleanup(&selection.owner, target.machine);
            continue;
        }
        let schema = data
            .iter_mut()
            .find(|s| s.declaration == declaration && s.carrier == ty)
            .ok_or_else(|| LoweringError::new(&selection.site, "selected cleanup needs the exact subject application"))?;
        if schema.multiplicity == crate::Multiplicity::Copy || schema.cleanup_target.is_some() || target.signature.result != ValueType::NoResult {
            return Err(LoweringError::new(
                &selection.site,
                "nominal cleanup requires one selected unit machine on owned data",
            ));
        }
        schema.cleanup_target = Some(target.machine);
    }
    super::write_only_cleanup::retain(&types, &mut data);
    for (instance, (contract, operator)) in instances.iter().zip(headers) {
        let sig = &*instance.signature;
        let machine_id = callees.local[&sig.machine][instance.application.arguments.as_slice()].machine;
        let id_for = |carrier: &ValueType| {
            types
                .id_for(carrier)
                .map_err(|error| LoweringError::new(&sig.site, error.to_string()))
        };
        let parameter_types = sig
            .parameters
            .iter()
            .map(|(_, ty)| id_for(slot_carrier(ty, &sig.machine)))
            .collect::<Result<Vec<_>, _>>()?;
        let result_type = (sig.result != ValueType::NoResult)
            .then(|| id_for(slot_carrier(&sig.result, &sig.machine)))
            .transpose()?;
        let (body, owners) = body::lower_body(
            prepared,
            &instance.tree,
            sig,
            &parameter_types,
            result_type,
            &body::Declarations {
                machine: machine_id,
                types: &types,
                data: &data,
                callees: &callees,
                requires: &contract.requires,
            },
        )?;
        crossing_owners.push(owners);
        machines.push(LoweredMachine {
            regions: regions::signature(sig, typed.generic_parameters.get(&sig.machine))?,
            operator,
            arguments: instance
                .application
                .arguments
                .iter()
                .enumerate()
                .map(|(i, arg)| {
                    if let Some(callback) = instance.callbacks.iter().find(|c| c.slot == i) {
                        let selected = callees
                            .local
                            .get(&callback.selection.machine)
                            .and_then(|entries| entries.get(&[][..]))
                            .ok_or_else(|| LoweringError::new(&sig.site, "selected callback has no lowered body"))?;
                        callback.lower_bound(&types, selected.machine)
                    } else if let ValueType::Implementation(id) = arg {
                        crate::declarations::members::evidence_argument(typed, instance, i, id)
                    } else if let ValueType::StaticInteger { carrier, value } = arg {
                        Ok(crate::StaticArgument::Integer {
                            carrier: types::scalar_carrier(carrier).map_err(|error| LoweringError::new(&sig.site, error.to_string()))?,
                            value: value.clone(),
                        })
                    } else {
                        id_for(arg).map(crate::StaticArgument::Type)
                    }
                })
                .collect::<Result<Vec<_>, _>>()?,
            positive_widths: instance.tree.positive_widths.clone(),
            id: machine_id,
            declaration: crate::identity::declaration(&sig.machine, &sig.site)?,
            contract,
            parameter_types,
            result_type,
            scalar_result: result_type
                .filter(|&ty| matches!(types.get(ty), Some(TypeShape::PrimitiveScalar(_) | TypeShape::QualifiedScalar { .. })))
                .map(|ty| ScalarResult { owner: machine_id, ty }),
            body,
        });
    }
    crate::machines::suspension::plan_package(
        &mut machines,
        &member_calls,
        &data,
        &types,
        &callees,
        &mut crossing_owners,
    )?;
    let domain_catalog = domain_schemas
        .into_iter()
        .map(|schema| Ok((schema.declaration, schema.lower(&types)?)))
        .collect::<Result<std::collections::BTreeMap<_, _>, LoweringError>>()?;
    mathematics.sort_unstable_by(|a, b| a.declaration.cmp(&b.declaration));
    data_templates.sort_unstable_by(|a, b| a.declaration.cmp(&b.declaration));
    conformances.sort_unstable_by(|a, b| a.declaration.cmp(&b.declaration));
    let types = types.finish(&domain_catalog)?;
    let mut domains = Vec::new();
    let mut domain_aliases = Vec::new();
    for declaration in domain_catalog.into_values() {
        match declaration {
            domains::DomainDeclaration::Named(domain) => domains.push(domain),
            domains::DomainDeclaration::Alias(alias) => domain_aliases.push(alias),
        }
    }
    let mut package = LoweredPackage {
        member_calls,
        conformances,
        callable_contracts,
        machine_templates,
        data_templates,
        mathematics,
        theorems,
        target_closure: crate::lowering::target::fold_target_closure(prepared)?,
        types,
        data,
        aliases,
        domains,
        domain_aliases,
        domain_templates,
        traits,
        machines,
    };
    crate::declarations::ranking::bind_component_edges(&mut package)?;
    Ok(package)
}
