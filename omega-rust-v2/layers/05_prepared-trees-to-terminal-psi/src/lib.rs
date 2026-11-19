//! Lower prepared source machines to owned, target-neutral Psi bodies.
#![forbid(unsafe_code)]

mod body;
mod types;
pub use body::{
    BlockId, ConstantOperation, EdgeId, EntryBlock, EntryDiagnostics, OperationId, Parameter, Place, PlaceId, PlaceOrigin, ReturnEdge, ScalarLiteral, Value,
    ValueId, ValueOrigin,
};
pub use types::{ScalarType, TypeError, TypeId, TypeShape, TypeTable};

use std::collections::HashMap;
use std::num::NonZeroU64;

use omega_parser::NodeKind;
use omega_preparation::{PreparedTrees, PreparedValueOrigin};
use omega_resolver::DeclarationId;
use omega_typer::{DeclaredPauseBounds, MachineEffects, Site, StaticValueDemand, TypedSignature, TypedTree, ValueType};

#[derive(Debug, Eq, PartialEq)]
pub struct LoweringError {
    pub site: Site,
    pub message: String,
}

impl LoweringError {
    fn new(
        site: &Site,
        message: impl Into<String>,
    ) -> Self {
        Self {
            site: site.clone(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for LoweringError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "{}: {}", self.site.file, self.message)
    }
}

impl std::error::Error for LoweringError {}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MachineId(NonZeroU64);
impl MachineId {
    pub fn get(self) -> u64 {
        self.0.get()
    }
}

/// The effective bounds of a machine with no runtime service calls or crash
/// alternatives. Published defaults come from typing, just like explicit bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineContract {
    pub public: bool,
    pub promises_termination: bool,
    pub has_empty_reach_ceiling: bool,
    pub has_empty_crash_ceiling: bool,
    pub pause_bounds: DeclaredPauseBounds,
}

#[derive(Debug, Eq, PartialEq)]
pub struct LoweredMachine {
    pub id: MachineId,
    pub declaration: DeclarationId,
    pub contract: MachineContract,
    pub parameter_types: Vec<TypeId>,
    pub result_type: Option<TypeId>,
    pub body: EntryBlock,
}

/// These identities belong to this package. The records own their data so a
/// consumer can read them after the prepared trees and source have been dropped.
#[derive(Debug, Eq, PartialEq)]
pub struct LoweredPackage {
    pub types: TypeTable,
    pub machines: Vec<LoweredMachine>,
}

fn site_at(
    tree: &TypedTree<'_>,
    node: usize,
) -> Site {
    let file = tree.resolved().file;
    Site {
        package: file.package.into(),
        file: file.path.into(),
        span: file.span(node),
    }
}

fn machine_contract(
    tree: &TypedTree<'_>,
    sig: &TypedSignature,
    effects: &MachineEffects,
) -> Result<MachineContract, LoweringError> {
    let file = tree.resolved().file;
    let node = &file.nodes[sig.node.node];
    if file.generic_parameters(sig.node.node).next().is_some() {
        return Err(LoweringError::new(
            &sig.site,
            "open generic machine requires a closed application before lowering",
        ));
    }
    if node.has_boundary_modifier() || file.child(sig.node.node, NodeKind::Body).is_none() {
        return Err(LoweringError::new(
            &sig.site,
            "boundary and bodyless declarations require callable declaration lowering",
        ));
    }
    for &child in &node.children {
        let kind = file.nodes[child].kind;
        match kind {
            NodeKind::Path
            | NodeKind::Params
            | NodeKind::Result
            | NodeKind::Body
            | NodeKind::Reaches
            | NodeKind::Crashes
            | NodeKind::Blocks
            | NodeKind::Suspends => {}
            NodeKind::Terminates if file.nodes[child].children.is_empty() => {}
            _ => return Err(LoweringError::new(&site_at(tree, child), format!("unsupported machine header: {kind:?}"))),
        }
    }
    if sig.requires
        || sig.has_guarantee
        || sig.satisfies.is_some()
        || !sig.multiplicity_bounds.is_empty()
        || sig.reach.as_ref().is_some_and(|s| !s.is_empty())
        || sig.crash_ceiling.as_ref().is_some_and(|s| !s.is_empty())
        || !effects.reach.is_empty()
        || !effects.invokes.is_empty()
        || !effects.callback_pause.is_empty()
        || effects.pause.block
        || effects.pause.suspend
        || effects.fixed_pause.block
        || effects.fixed_pause.suspend
    {
        return Err(LoweringError::new(&sig.site, "machine requires contract or effects lowering"));
    }
    Ok(MachineContract {
        public: node.has_public_modifier(),
        promises_termination: sig.terminates,
        has_empty_reach_ceiling: sig.reach.is_some(),
        has_empty_crash_ceiling: sig.crash_ceiling.is_some(),
        pause_bounds: sig.pause,
    })
}

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
    let mut signatures = Vec::new();
    for tree in &trees {
        let file = tree.resolved().file;
        for &child in &file.nodes[0].children {
            let kind = file.nodes[child].kind;
            if kind == NodeKind::Machine {
                let id = tree
                    .resolved()
                    .declaration_at(child)
                    .ok_or_else(|| LoweringError::new(&site_at(tree, child), "machine has no resolved declaration"))?;
                let sig = signature_by_declaration
                    .get(id)
                    .copied()
                    .filter(|sig| sig.node == file.node_id(child))
                    .ok_or_else(|| LoweringError::new(&site_at(tree, child), "machine has no local typed signature"))?;
                signatures.push(sig);
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
    if !prepared.target_observations().is_empty() {
        // Folded values and signatures can depend on a selection made in an
        // earlier typing pass. Until this product carries the closure, dropping
        // it would authorize reuse under a different target.
        let recorded = |origin: PreparedValueOrigin| {
            prepared
                .dependencies()
                .inputs(&origin)
                .is_some_and(|inputs| !inputs.target_observations.is_empty())
        };
        let site = typed
            .constants
            .iter()
            .find(|c| recorded(PreparedValueOrigin::Constant(c.declaration.clone())))
            .map(|c| &c.initializer.expression)
            .or_else(|| {
                typed
                    .evaluated_indices
                    .iter()
                    .find(|(r, _)| recorded(PreparedValueOrigin::StaticIndex(StaticValueDemand::Closed(r.clone()))))
                    .map(|(r, _)| &r.expression)
            })
            .or_else(|| {
                prepared
                    .specialized_indices()
                    .iter()
                    .find(|v| {
                        recorded(PreparedValueOrigin::StaticIndex(StaticValueDemand::Specialized {
                            request: v.request.clone(),
                            arguments: v.arguments.clone(),
                        }))
                    })
                    .map(|v| &v.request.expression)
            })
            .expect("each observed preparation input belongs to a retained demand");
        return Err(LoweringError::new(site, "target-dependent values require a portable target closure"));
    }
    signatures.sort_unstable_by(|a, b| a.machine.cmp(&b.machine));
    let effects: HashMap<_, _> = typed
        .machine_effects
        .iter()
        .map(|e| (&e.machine, e.as_ref()))
        .collect();
    let mut headers = Vec::with_capacity(signatures.len());
    let mut carriers = Vec::new();
    for sig in &signatures {
        let tree = &trees[sig.node.file];
        let effect = effects
            .get(&sig.machine)
            .ok_or_else(|| LoweringError::new(&sig.site, "machine has no normalized effect row"))?;
        headers.push(machine_contract(tree, sig, effect)?);
        for (span, carrier) in &sig.parameters {
            let mut site = sig.site.clone();
            site.span = *span;
            carriers.push((carrier, site));
        }
        if sig.result != ValueType::NoResult {
            let site = tree
                .resolved()
                .file
                .child(sig.node.node, NodeKind::Result)
                .map_or_else(|| sig.site.clone(), |node| site_at(tree, node));
            carriers.push((&sig.result, site));
        }
    }
    let types = TypeTable::from_carriers(carriers.iter().map(|(carrier, _)| *carrier))
        .map_err(|(index, error)| LoweringError::new(&carriers[index].1, error.to_string()))?;
    let mut machines = Vec::with_capacity(signatures.len());
    for (sig, contract) in signatures.into_iter().zip(headers) {
        let id_for = |carrier: &ValueType| {
            types
                .id_for(carrier)
                .map_err(|error| LoweringError::new(&sig.site, error.to_string()))
        };
        let parameter_types = sig
            .parameters
            .iter()
            .map(|(_, ty)| id_for(ty))
            .collect::<Result<Vec<_>, _>>()?;
        let result_type = (sig.result != ValueType::NoResult)
            .then(|| id_for(&sig.result))
            .transpose()?;
        let body = body::lower_body(prepared, sig, &parameter_types, result_type)?;
        let ordinal = u64::try_from(machines.len())
            .ok()
            .and_then(|n| n.checked_add(1))
            .and_then(NonZeroU64::new)
            .ok_or_else(|| LoweringError::new(&sig.site, "too many Psi machines"))?;
        machines.push(LoweredMachine {
            id: MachineId(ordinal),
            declaration: sig.machine.clone(),
            contract,
            parameter_types,
            result_type,
            body,
        });
    }
    Ok(LoweredPackage { types, machines })
}
