//! One source package through the ordinary path: resolve, prepare, lower.
//!
//! The typed obligations are copied out of the prepared trees, `lower_package`
//! runs on those same trees, and the prepared and resolved packages are dropped
//! before the caller inspects anything. The Psi product therefore has to carry
//! what the comparison needs on its own.
use omega_lowering::{
    BlockTerminator, LoweredMachine, LoweredPackage, MachineBody, PlaceId, PlaceOrigin, PsiOperation, ReturnEdge, Site, ValueId, ValueOrigin,
};
use omega_typed::TypingObligation;
use tree_handoff_consumer::obligation_report;

pub const PACKAGE: &str = "app";
pub const FILE: &str = "obligations.omg";

pub struct Lowered<'source> {
    source: &'source str,
    obligations: Vec<TypingObligation>,
    pub package: LoweredPackage,
}

pub fn lower(source: &str) -> Lowered<'_> {
    let (obligations, product) = {
        let resolved = crate::sources::resolve(&[(FILE, source)]);
        let prepared = omega_preparation::prepare_source_package(&resolved).unwrap_or_else(|error| panic!("{source}: preparation failed: {error:?}"));
        let obligations = prepared.typed().obligations.clone();
        let product = crate::sources::lower_prepared(&prepared, &[(FILE, source)]);
        drop(prepared);
        drop(resolved);
        (obligations, product)
    };
    let package = product.unwrap_or_else(|error| panic!("{source}: lowering failed: {error:?}"));
    Lowered { source, obligations, package }
}

impl Lowered<'_> {
    /// Every prepared obligation of this kind, in producer order.
    pub fn obligations(
        &self,
        kind: &str,
    ) -> Vec<&TypingObligation> {
        assert!(obligation_report::KINDS.contains(&kind), "unknown obligation kind {kind}");
        self.obligations
            .iter()
            .filter(|obligation| obligation_report::name(&obligation.kind) == kind)
            .collect()
    }

    /// The source text of a site in this package's only file.
    pub fn spelling(
        &self,
        site: &Site,
    ) -> &str {
        assert_eq!((site.package.as_str(), site.file.as_str()), (PACKAGE, FILE));
        std::str::from_utf8(
            site.span
                .slice(self.source.as_bytes())
                .expect("site lies in the source"),
        )
        .unwrap()
    }

    pub fn machine(
        &self,
        path: &str,
    ) -> &LoweredMachine {
        let found: Vec<_> = self
            .package
            .machines
            .iter()
            .filter(|machine| machine.declaration.path == path)
            .collect();
        let [machine] = found.as_slice() else {
            panic!("expected one lowered machine {path}, found {}", found.len())
        };
        machine
    }
}

/// The one operation whose retained diagnostic site is exactly `site`.
pub fn operation_at<'a>(
    body: &'a MachineBody,
    site: &Site,
) -> &'a PsiOperation {
    assert_eq!(body.diagnostics.operations.len(), body.operations.len());
    let found: Vec<_> = body
        .operations
        .iter()
        .filter(|operation| &body.diagnostics.operations[operation.id.index()] == site)
        .collect();
    let [operation] = found.as_slice() else {
        panic!("expected one operation at {site:?}, found {found:?}")
    };
    operation
}

pub fn value_producer(
    body: &MachineBody,
    value: ValueId,
) -> &PsiOperation {
    let ValueOrigin::Operation(id) = body.values[value.index()].origin else {
        panic!("value {value:?} is not an operation result")
    };
    &body.operations[id.index()]
}

pub fn place_producer(
    body: &MachineBody,
    place: PlaceId,
) -> &PsiOperation {
    let PlaceOrigin::Operation(id) = body.places[place.index()].origin else {
        panic!("place {place:?} is not an operation result")
    };
    &body.operations[id.index()]
}

/// Every return edge of the body.
pub fn returns(body: &MachineBody) -> Vec<ReturnEdge> {
    body.blocks
        .iter()
        .filter_map(|block| match block.terminator {
            BlockTerminator::Return { result, .. } => Some(result),
            _ => None,
        })
        .collect()
}
