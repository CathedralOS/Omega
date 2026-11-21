//! Element bounds of an exclusive nominal-element view, read from published Psi alone.
//!
//! Each extraction or store through `p[i]` on `p: @mut [T]` owes `i < len`,
//! where `len` is an `ElementViewLength` observation of that same view
//! (`wiki/spec/terminal-psi/byte_views.md`, typed slices). An entry clause
//! `i < p.length` publishes the same count as a contract term on the formal `p`.
//! This reader relates the two by exact identity: the bound's view must be the
//! formal the clause measures, and its selector must be the formal the clause
//! compares. It is not the checker's verdict. It reasons about no arithmetic,
//! branch fact or guarantee, so an unmatched bound is still owed, not refuted.
use omega_psi::{
    BlockTerminator, BoundsObligationId, LoweredMachine, LoweredPackage, OperationId, OperationKind, OperationResult, Parameter, PlaceOrigin, PlaceProjection,
    PredicateOperator, PredicateStep, ProjectedPlace, RuntimeIndex, TypeShape, ValueId, ValueOrigin,
};

/// One element bound owed at an extraction or store occurrence. A displacement
/// extracts and stores through one occurrence, so both operations share it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewElementBound {
    pub operations: Vec<OperationId>,
    /// Formal position of the view the element belongs to.
    pub view: usize,
    pub selector: ValueId,
    pub obligation: BoundsObligationId,
    pub length: ValueId,
    /// Positions in `contract.requires` of clauses that state this exact bound:
    /// `s < v.length` or `v.length > s` over the bound's own view and selector formals.
    pub stated_by: Vec<usize>,
}

/// Read every element bound of a machine's view extractions and stores,
/// refusing a bound whose length is not a dominating count of its own view.
/// A length must be the direct result of its observation; one forwarded
/// through a block parameter is refused rather than traced.
pub fn view_element_bounds(
    package: &LoweredPackage,
    machine: &LoweredMachine,
) -> Result<Vec<ViewElementBound>, &'static str> {
    let body = &machine.body;
    let dominators = dominators(body)?;
    let block_of = |operation: usize| {
        body.blocks
            .iter()
            .position(|block| block.operations.contains(&operation))
    };
    let formal_of_place = |place| {
        body.parameters
            .iter()
            .position(|p| *p == Parameter::Structural(place))
    };
    let formal_of_value = |value| {
        body.parameters
            .iter()
            .position(|p| *p == Parameter::Scalar(value))
    };
    let mut bounds: Vec<ViewElementBound> = Vec::new();
    for (block, position) in body
        .blocks
        .iter()
        .enumerate()
        .flat_map(|(index, block)| block.operations.clone().map(move |op| (index, op)))
    {
        let op = &body.operations[position];
        let place: &ProjectedPlace = match &op.kind {
            OperationKind::BorrowedMove { source } => source,
            OperationKind::StructuralStore { destination, .. } => destination,
            _ => continue,
        };
        let [
            PlaceProjection::Referent,
            PlaceProjection::ViewElement {
                index: RuntimeIndex { selector, obligation },
                length,
            },
        ] = place.path.as_slice()
        else {
            continue;
        };
        let view = formal_of_place(place.root)
            .filter(|_| body.places[place.root.index()].origin == PlaceOrigin::Parameter)
            .ok_or("an element bound is rooted outside the machine's view formals")?;
        if !matches!(package.types.get(machine.parameter_types[view]), Some(TypeShape::MutableElementView { element: omega_psi::ViewElement::Nominal(_) })) {
            return Err("an element bound is rooted at a formal that is not an exclusive element view");
        }
        let ValueOrigin::Operation(observed) = body.values[length.index()].origin else {
            return Err("an element bound's length is not an operation result");
        };
        let observation = &body.operations[observed.index()];
        if observation.result != OperationResult::Scalar(*length) {
            return Err("an element bound's length disagrees with its producing operation");
        }
        let OperationKind::ElementViewLength { source } = observation.kind else {
            return Err("an element bound's length is not an element count observation");
        };
        if source != place.root {
            return Err("an element bound's length observes another view");
        }
        let dominates = match block_of(observed.index()) {
            Some(home) if home == block => observed.index() < position,
            Some(home) => dominators[block][home],
            None => false,
        };
        if !dominates {
            return Err("an element bound's length does not dominate its occurrence");
        }
        if let Some(bound) = bounds.iter_mut().find(|bound| bound.obligation == *obligation) {
            if (bound.view, bound.selector, bound.length) != (view, *selector, *length) {
                return Err("two occurrences share one bound obligation");
            }
            bound.operations.push(op.id);
            continue;
        }
        let selector_formal = formal_of_value(*selector);
        let stated_by = machine
            .contract
            .requires
            .iter()
            .enumerate()
            .filter(|(_, clause)| selector_formal.is_some_and(|s| states_bound(&clause.predicate, view, s)))
            .map(|(index, _)| index)
            .collect();
        bounds.push(ViewElementBound {
            operations: vec![op.id],
            view,
            selector: *selector,
            obligation: *obligation,
            length: *length,
            stated_by,
        });
    }
    Ok(bounds)
}

/// `dominators[b][d]`: every path from the entry block to block `b` passes
/// through block `d`. Computed by the iterative set intersection over the
/// published jump and conditional edges; other terminators have no successor.
fn dominators(body: &omega_psi::MachineBody) -> Result<Vec<Vec<bool>>, &'static str> {
    let count = body.blocks.len();
    let mut predecessors = vec![Vec::new(); count];
    for (from, block) in body.blocks.iter().enumerate() {
        let targets: Vec<_> = match &block.terminator {
            BlockTerminator::Jump(edge) => vec![edge.target],
            BlockTerminator::Conditional { when_true, when_false, .. } => vec![when_true.target, when_false.target],
            _ => Vec::new(),
        };
        for target in targets {
            let to = target
                .get()
                .checked_sub(1)
                .filter(|&to| to < count)
                .ok_or("an edge targets a missing block")?;
            predecessors[to].push(from);
        }
    }
    let mut dominators = vec![vec![true; count]; count];
    if let Some(entry) = dominators.first_mut() {
        *entry = (0..count).map(|block| block == 0).collect();
    }
    let mut changed = true;
    while changed {
        changed = false;
        for block in 1..count {
            let mut next = vec![!predecessors[block].is_empty(); count];
            for &from in &predecessors[block] {
                for (slot, &held) in next.iter_mut().zip(&dominators[from]) {
                    *slot &= held;
                }
            }
            next[block] = true;
            if next != dominators[block] {
                dominators[block] = next;
                changed = true;
            }
        }
    }
    Ok(dominators)
}

/// `selector < view.length` or `view.length > selector`, with the count step on the view formal.
fn states_bound(
    predicate: &[PredicateStep],
    view: usize,
    selector: usize,
) -> bool {
    use PredicateStep::{Apply, ElementViewLength, Formal};
    match predicate {
        [
            Formal(s),
            Formal(v),
            ElementViewLength,
            Apply(PredicateOperator::LessThan),
        ] => (*s, *v) == (selector, view),
        [
            Formal(v),
            ElementViewLength,
            Formal(s),
            Apply(PredicateOperator::GreaterThan),
        ] => (*s, *v) == (selector, view),
        _ => false,
    }
}

/// Read which formal provided the element count returned by a straight-line body.
pub fn returned_element_count_formal(machine: &LoweredMachine) -> Option<usize> {
    returned_element_count(machine).map(|(formal, _)| formal)
}

fn returned_element_count(machine: &LoweredMachine) -> Option<(usize, ValueId)> {
    let [block] = machine.body.blocks.as_slice() else { return None };
    let BlockTerminator::Return {
        result: omega_psi::ReturnEdge::Scalar(result),
        ..
    } = block.terminator
    else {
        return None;
    };
    let count = machine.body.operations.get(block.operations.clone())?
        .iter()
        .find(|op| op.result == OperationResult::Scalar(result))?;
    let OperationKind::ElementViewLength { source } = count.kind else {
        return None;
    };
    machine
        .body
        .parameters
        .iter()
        .position(|p| *p == Parameter::Structural(source))
        .map(|formal| (formal, result))
}

/// Read the access and exact data declaration of the formal whose live count is returned.
pub fn returned_nominal_view_count<'a>(
    package: &'a LoweredPackage,
    machine: &LoweredMachine,
) -> Option<(usize, omega_psi::ReferenceAccess, &'a omega_psi::DeclarationId)> {
    use omega_psi::{ReferenceAccess, ScalarType, ViewElement};
    let (formal, result) = returned_element_count(machine)?;
    let Parameter::Structural(source) = *machine.body.parameters.get(formal)? else { return None };
    let descriptor = machine.body.places.get(source.index())?;
    if machine.parameter_types.get(formal) != Some(&descriptor.ty) { return None; }
    let (access, element) = match package.types.get(descriptor.ty)? {
        TypeShape::SharedElementView { element: ViewElement::Nominal(data) } => (ReferenceAccess::Shared, *data),
        TypeShape::MutableElementView { element: ViewElement::Nominal(data) } => (ReferenceAccess::Mutable, *data),
        _ => return None,
    };
    let result_type = machine.body.values.get(result.index())?.ty;
    if Some(result_type) != machine.result_type
        || package.types.get(result_type) != Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 64 })) {
        return None;
    }
    let data = package.data.iter().find(|data| data.id == element)?;
    Some((formal, access, &data.declaration))
}
