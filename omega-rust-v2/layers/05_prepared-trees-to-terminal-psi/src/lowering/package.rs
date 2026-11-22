use crate::declarations::domains::SelectedPolicies;
use crate::declarations::policy_answer::PolicyUnit;
use crate::declarations::{data, domains, machine::machine_header};
use crate::declarations::regions::{self, slot_carrier};
use crate::lowering::pending::{PendingPolicyDomain, PendingPolicyPackage, PolicyIntroduction};
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
///
/// A domain that selects an access policy is refused here, with the reason its
/// policy is unanswered. [`lower_pending_policies`] lowers such a package for
/// the driver that answers its policies.
pub fn lower_package(prepared: &PreparedTrees<'_>) -> Result<LoweredPackage, LoweringError> {
    decided(lower(prepared, None, SelectedPolicies::Refuse)?)
}

/// Lower a package whose access policies are not answered yet, and say which
/// of its domains wait for one.
///
/// Who may issue `domain i32::Locked access Rule` is what an execution of
/// `Rule::plan` returns. This function runs nothing and reads no answer, so
/// it decides no issuer. It lowers every body and every other declaration,
/// leaves the row of each such domain out of the package, and returns that
/// row with no issuer beside it. Each row lists the casts that add the domain
/// to a value that lacks it, such as `x as +Locked`, with the machine that
/// owns the cast. A value that already carries the domain moves through
/// formals, calls and results, and `x as -Locked` gives it up; none of those
/// is listed.
///
/// The driver completes the package: it authenticates each executed plan,
/// refuses a cast that adds a domain whose plan admits no issuer, and writes
/// the rows with their issuers.
///
/// An `access` clause on a domain alias or on a carrier-generic domain is
/// refused. Neither has a row that states issuers, so an answered plan would
/// be dropped.
pub fn lower_pending_policies(prepared: &PreparedTrees<'_>) -> Result<PendingPolicyPackage, LoweringError> {
    lower(prepared, None, SelectedPolicies::Defer)
}

/// The package of a lowering that deferred nothing.
fn decided(lowered: PendingPolicyPackage) -> Result<LoweredPackage, LoweringError> {
    let (package, pending) = lowered.into_parts();
    // `Refuse` and a policy unit leave no domain waiting. One that does wait has no issuer to publish.
    match pending.first() {
        None => Ok(package),
        Some(domain) => Err(LoweringError::new(&domain.selection, "domain access policy has no issuer decision")),
    }
}

/// Lower the closed execution of one selected access policy: the selected
/// `plan` machine and every body its calls reach, in the package that selects
/// it. For `domain i32::Locked access Rule` that is `Rule::plan` and the
/// toolchain's `access\new_plan`, each under the package that declares it,
/// with the data they store.
///
/// This is the ordinary package lowering with two differences. Only the
/// selected machine is a root, so no other machine of the package is in the
/// unit unless the plan calls it. A domain that selects an access policy is
/// left out, along with domains and aliases that require it. The full package
/// retains all of them so its driver can decide every introduction. The unit
/// runs nothing, validates no plan and grants no issuance.
///
/// The returned [`PolicyUnit`] keeps the package together with the invocation
/// preparation froze for this selection. Its package cannot be replaced, so
/// the checker and the interpreter judge and run exactly these bodies, and the
/// driver can match an execution receipt to them by address.
///
/// `None` means the selected machine has no closed body here: it is still an
/// open template, for example because its body holds a value at its region
/// that has no region slot. A refusal inside the closure is an error.
pub fn lower_selected_policy_unit(
    prepared: &PreparedTrees<'_>,
    selection: &omega_typed::AccessPolicySelection,
) -> Result<Option<PolicyUnit>, LoweringError> {
    if !prepared.typed().access_policies.contains(selection) {
        return Err(LoweringError::new(&selection.site, "access policy selection is absent from the prepared package"));
    }
    let unit = decided(lower(prepared, Some(selection), SelectedPolicies::Refuse)?)?;
    let plan = crate::identity::declaration(&selection.plan_machine, &selection.site)?;
    let mut closed = unit
        .machines
        .iter()
        .enumerate()
        .filter(|(_, machine)| machine.declaration == plan && machine.arguments.is_empty());
    let Some((position, _)) = closed.next() else {
        return Ok(None);
    };
    // One entry: an answer names the body that ran by its address.
    if closed.next().is_some() {
        return Err(LoweringError::new(&selection.site, "selected access policy has more than one closed body"));
    }
    PolicyUnit::new(prepared, selection, unit, position).map(Some)
}

fn lower(
    prepared: &PreparedTrees<'_>,
    unit: Option<&omega_typed::AccessPolicySelection>,
    selected: SelectedPolicies,
) -> Result<PendingPolicyPackage, LoweringError> {
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
    let mut catalog_declarations = Vec::new();
    let library_trees: Vec<_> = typed
        .library
        .into_iter()
        .flat_map(|library| library.trees())
        .collect();
    // Each package's own declarations decide which of its records keep a
    // type row while taking lifetime arguments.
    let region_data = regions::RegionData::of_package(typed, &trees);
    let library_regions = typed
        .library
        .map_or_else(|| regions::RegionData::of_package(typed, &[]), |library| regions::RegionData::of_package(library, &library_trees));
    let mut data_schemas = Vec::new();
    let mut aliases = Vec::new();
    let unavailable_domains = unit.map(|_| domains::policy_unit_exclusions(typed)).unwrap_or_default();
    let mut domain_schemas = Vec::new();
    let mut domain_templates = Vec::new();
    // The domains that select a policy, in the order their files declare them.
    let mut pending_domains = Vec::new();
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
                if sig.catalog_supplied {
                    catalog_declarations.push(crate::identity::declaration(id, &sig.site)?);
                    continue;
                }
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
                if let Some(template) = crate::declarations::data_templates::read(prepared, tree, child, &mut declarations, &region_data)? {
                    data_templates.push(template);
                    continue;
                }
                match data::read_data(prepared, tree, child, &mut declarations, &region_data)? {
                    data::DataDeclaration::Schema(schema) => data_schemas.push(schema),
                    data::DataDeclaration::Alias(alias) => aliases.push(alias),
                }
            } else if kind == NodeKind::Conformance {
                conformances.push(crate::declarations::members::conformance(typed, tree, child, &mut declarations)?);
            } else if kind == NodeKind::Trait {
                trait_schemas.push(crate::declarations::traits::read_trait(prepared, tree, child, &signature_by_declaration)?);
            } else if kind == NodeKind::Domain {
                if tree.resolved().declaration_at(child).is_some_and(|domain| unavailable_domains.contains(domain)) {
                    continue;
                }
                let schema = domains::read_domain(prepared, tree, child, selected)?;
                if schema.policy.is_some() {
                    pending_domains.push(schema.declaration);
                }
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
    if let Some(selection) = unit {
        let required = typed.conformance_targets.get(&selection.policy)
            .ok_or_else(|| LoweringError::new(&selection.site, "selected policy lacks its checked trait application"))?;
        if !trait_schemas.iter().any(|schema| schema.declaration == &required.declaration) {
            let (tree, node) = library_trees.iter().find_map(|tree| {
                tree.resolved().declaration_nodes().find(|(_, declaration)| *declaration == &required.declaration)
                    .map(|(node, _)| (tree, node))
            }).ok_or_else(|| LoweringError::new(&selection.site, "policy execution unit lacks the selected trait definition"))?;
            let signatures = tree.forest().signatures.iter().map(|signature| (&signature.machine, signature.as_ref())).collect();
            trait_schemas.push(crate::declarations::traits::read_trait(prepared, tree, node, &signatures)?);
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
    // A machine can hold a record of this package and one of its library,
    // such as the toolchain's `TypeSchema<'t>`. Each package judged its own.
    let machine_regions = &region_data.with(&library_regions);
    let only_root = unit.map(|selection| &selection.plan_machine);
    let instances = super::applications::plan(prepared, &signatures, &trees, &library_trees, typed, machine_regions, only_root)?;
    let members = crate::declarations::members::plan(typed, &trees, &instances, &signature_by_declaration)?;
    // A unit holds closed bodies only. The package's open templates stay with the package.
    let machine_templates = if unit.is_some() {
        Vec::new()
    } else {
        crate::declarations::machine_templates::plan(prepared, &trees, &signatures, &instances, &trait_schemas)?
    };
    let effects: HashMap<_, _> = typed
        .machine_effects
        .iter()
        .map(|e| (&e.machine, e.as_ref()))
        .collect();
    let stored_ceilings = super::callbacks::stored_ceilings(typed);
    let mut headers = Vec::with_capacity(instances.len());
    let mut carriers = Vec::new();
    data_schemas.sort_unstable_by(|a, b| a.declaration.cmp(b.declaration));
    let applications = super::applied_data::collect(typed, &data_templates, &region_data);
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
    // An imported machine can use a domain only inside its body. Its caller
    // need not expose that domain in a signature or in its own expressions.
    let mut demands = std::collections::HashSet::new();
    let mut collect_domains = |ty: &ValueType| {
        for part in ty.components() {
            demands.extend(part.domains().iter().map(|atom| atom.declaration.clone()));
        }
    };
    for instance in &instances {
        let sig = &instance.signature;
        for (_, parameter) in &sig.parameters { collect_domains(parameter); }
        collect_domains(&sig.result);
        if !std::ptr::eq(instance.tree.forest(), typed) {
            let file = instance.tree.resolved().file;
            let mut pending = vec![sig.node.node];
            while let Some(node) = pending.pop() {
                if node != sig.node.node && matches!(file.nodes[node].kind, NodeKind::Machine | NodeKind::AsmMachine) {
                    continue;
                }
                pending.extend(&file.nodes[node].children);
                for record in instance.tree.semantics_at(node) {
                    if let omega_typed::NodeSemantics::Expression(expr) = record { collect_domains(&expr.ty); }
                }
            }
        }
    }
    for ty in typed.expressions.iter().map(|e| &e.ty)
        .chain(typed.record_fields.values().flat_map(|fields| fields.iter().map(|field| &field.ty)))
        .chain(typed.type_alias_targets.values())
    {
        collect_domains(ty);
    }
    for tree in &library_trees {
        for (node, declaration) in tree.resolved().declaration_nodes() {
            if declaration.kind == NodeKind::Domain && demands.contains(declaration) {
                if tree.resolved().file.child(node, NodeKind::DroppedBy).is_some() {
                    return Err(LoweringError::new(&site_at(tree, node), "imported domain needs its defining cleanup machine in the package"));
                }
                let schema = domains::read_domain(prepared, tree, node, domains::SelectedPolicies::Refuse)?;
                domain_schemas.push(schema);
            }
        }
    }
    domain_schemas.sort_unstable_by(|a, b| a.declaration.cmp(b.declaration));
    for domain in &domain_schemas {
        if domain.width_binder().is_none() {
            carriers.push((domain.carrier, domain.site.clone()));
        }
    }
    // The data rows keep this position in the type table. They are added once
    // the machine bodies below have named the library data they store.
    let data_carriers_at = carriers.len();
    // A body can hold data that no carrier row mentions, such as a local of a
    // library sum. Its type still needs that declaration's schema.
    let mut body_types = Vec::new();
    let no_rows = std::collections::HashSet::new();
    let retained = instances.iter().flat_map(|i| i.tree.guarded_rows.iter().copied()).collect();
    // Each cast that adds a pending domain: that domain's position in
    // `pending_domains`, the instance that owns the cast, and the cast.
    let mut introductions = Vec::new();
    for (owner, instance) in instances.iter().enumerate() {
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
        // The rows of an imported body are its library's, and so are their indices.
        let own = std::ptr::eq(tree.forest(), typed);
        crate::declarations::crash_routes::check_invocations(tree.forest(), sig, if own { &retained } else { &no_rows })?;
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
                // Typing records one establishment for each domain a cast adds
                // to its operand. A value that arrives with the domain has none.
                if let omega_typed::NodeSemantics::Obligation(row) = record
                    && let omega_typed::ObligationKind::EstablishDomain(atom) = &row.kind
                    && let Some(domain) = pending_domains.iter().position(|pending| **pending == atom.declaration)
                {
                    introductions.push((domain, owner, row.site.clone()));
                }
                // Restoring an indexed hole can branch on two captured selectors
                // even when the source contains no Boolean expression.
                if file.nodes[node].kind == NodeKind::Index && matches!(record,
                    omega_typed::NodeSemantics::Obligation(row) if matches!(row.kind, omega_typed::ObligationKind::RestoreBorrowed { .. })
                ) {
                    carriers.push((&ValueType::Bool, site_at(tree, node)));
                }
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
                || tree.forest().complete_case_patterns.contains_key(&file.node_id(node))
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
                body_types.push(carrier);
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
    let local_fields = data_schemas.iter().flat_map(|data| {
        data.fields()
            .filter_map(|field| Some(data.field_carrier(field).ok()?.0))
    });
    let stored = carriers
        .iter()
        .map(|(carrier, _)| *carrier)
        .chain(body_types)
        .chain(local_fields);
    let library_data = demanded_library_data(prepared, &library_trees, &library_regions, stored)?;
    data_schemas.extend(library_data);
    data_schemas.sort_unstable_by(|a, b| a.declaration.cmp(b.declaration));
    let case_owners: std::collections::HashSet<_> = typed.case_owners.values().collect();
    let mut data_carriers = Vec::new();
    for data in &data_schemas {
        // Normal cleanup can test an intact sum's discriminator even when
        // the machine has no authored Boolean expression.
        if case_owners.contains(data.declaration) {
            data_carriers.push((&ValueType::Bool, data.site.clone()));
        }
        data_carriers.push((&data.carrier, data.site.clone()));
        data_carriers.extend(data.value_indices.iter().map(|ty| (*ty, data.site.clone())));
        for field in data.fields() {
            data_carriers.push((data.field_carrier(field)?.0, field.site.clone()));
        }
    }
    carriers.splice(data_carriers_at..data_carriers_at, data_carriers);
    let mut types = LoweringTypes::from_declarations(
        carriers.iter().map(|(carrier, _)| *carrier),
        data_schemas
            .iter()
            .map(|data| data.declaration.clone())
            .collect(),
        data_schemas.iter().map(|data| data.lifetime_count).collect(),
        &applications,
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
                // A body with no static argument keeps its region binders. Its
                // telescope is the defining forest's, which for an imported
                // body is the library's.
                regions: instance
                    .application
                    .arguments
                    .is_empty()
                    .then(|| instance.tree.forest().generic_parameters.get(&sig.machine))
                    .flatten()
                    .map_or(0, |telescope| {
                        let lifetimes = telescope.parameters.iter().filter(|p| p.kind == NodeKind::LifetimeParam);
                        lifetimes.count()
                    }),
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
    // The cast lowerer adds a pending domain only where the roster says so.
    types.report_policy_introductions(
        introductions
            .iter()
            .map(|(domain, owner, site)| (pending_domains[*domain].clone(), instances[*owner].signature.machine.clone(), site.clone()))
            .collect(),
    );
    super::write_only_cleanup::retain(&types, &mut data);
    crate::machines::ownership::admit_sequence_elements(&types, &data)?;
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
            regions: regions::signature(sig, instance.tree.forest().generic_parameters.get(&sig.machine), machine_regions)?,
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
    let mut pending = Vec::with_capacity(pending_domains.len());
    for declaration in domain_catalog.into_values() {
        match declaration {
            domains::DomainDeclaration::Named { row, policy: None } => domains.push(domains::unrestricted(row)),
            // The package publishes no row for this domain until its plan is answered.
            domains::DomainDeclaration::Named { row, policy: Some(selection) } => {
                let position = pending_domains.iter().position(|pending| **pending == selection.domain);
                let position = position.ok_or_else(|| LoweringError::new(&selection.site, "pending access policy has no declared domain"))?;
                let mut casts: Vec<_> = introductions
                    .iter()
                    .filter(|(domain, _, _)| *domain == position)
                    .map(|(_, owner, site)| PolicyIntroduction {
                        machine: machines[*owner].id,
                        declaration: machines[*owner].declaration.clone(),
                        site: site.clone(),
                    })
                    .collect();
                casts.sort_unstable_by(|left, right| (left.machine.get(), &left.site).cmp(&(right.machine.get(), &right.site)));
                pending.push((
                    position,
                    PendingPolicyDomain {
                        source: selection.domain.clone(),
                        selection: selection.site.clone(),
                        row,
                        introductions: casts,
                    },
                ));
            }
            domains::DomainDeclaration::Alias(alias) => domain_aliases.push(alias),
            domains::DomainDeclaration::Template(row) => domain_templates.push(row),
        }
    }
    domain_templates.sort_unstable_by(|a, b| a.declaration.cmp(&b.declaration));
    pending.sort_unstable_by_key(|(position, _)| *position);
    catalog_declarations.sort_unstable();
    let mut package = LoweredPackage {
        catalog_declarations,
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
    Ok(PendingPolicyPackage::new(package, pending.into_iter().map(|(_, domain)| domain).collect()))
}

/// Read the library data that `stored` types hold, each from the file that
/// declares it. `machine keep(e: Exposure) -> Exposure { e }` needs the
/// toolchain's `Exposure`. `FieldAccess` also needs the `Exposure`,
/// `ExternalRead` and `AtomicOperations` its cases store.
///
/// A type demands a declaration by naming it, so a library declaration that
/// no closed body or local declaration names is not read. An open template
/// names library types by declaration and contributes no type here, so a
/// package of templates publishes none. Each schema keeps the package and
/// path of its defining file, and its `requires` clauses come from that
/// file's own typed facts.
fn demanded_library_data<'a: 'input, 'input>(
    prepared: &'a PreparedTrees<'_>,
    library_trees: &'a [omega_typed::TypedTree<'a>],
    library_regions: &'a regions::RegionData<'a>,
    stored: impl Iterator<Item = &'input ValueType>,
) -> Result<Vec<data::DataSchema<'a>>, LoweringError> {
    let named = |carrier: &'input ValueType| {
        carrier
            .components()
            .filter_map(|part| Some(part.nominal_parts()?.0))
    };
    // Most bodies name the same few types many times, and most of those are local.
    let mut pending: Vec<_> = stored.flat_map(named).collect();
    pending.sort_unstable();
    pending.dedup();
    let mut declared: Vec<_> = library_trees
        .iter()
        .flat_map(|tree| {
            let nodes = tree.resolved().declaration_nodes();
            nodes
                .filter(|(_, id)| id.kind == NodeKind::Data)
                .map(move |(node, id)| (id, tree, node))
        })
        .collect();
    declared.sort_unstable_by_key(|&(id, _, _)| id);
    let mut schemas: Vec<data::DataSchema<'a>> = Vec::new();
    while let Some(id) = pending.pop() {
        // Local data and local templates are not declared by the library.
        let Ok(row) = declared.binary_search_by_key(&id, |&(id, _, _)| id) else {
            continue;
        };
        if schemas.iter().any(|schema| schema.declaration == id) {
            continue;
        }
        let (_, tree, node) = declared[row];
        let mut nested = Vec::new();
        let data::DataDeclaration::Schema(schema) = data::read_data(prepared, tree, node, &mut nested, library_regions)? else {
            return Err(LoweringError::new(&site_at(tree, node), "imported nominal type has no data declaration"));
        };
        if !nested.is_empty() {
            return Err(LoweringError::new(&schema.site, "imported data requires its nested declarations"));
        }
        for field in schema.fields() {
            pending.extend(named(schema.field_carrier(field)?.0));
        }
        schemas.push(schema);
    }
    Ok(schemas)
}
