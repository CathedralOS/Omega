use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use symbols::SymbolHandle;

pub(crate) use checked_trees::CapturedPlace as BorrowAccessPlace;

/// The declaration's outer reference grants its referent namespace directly.
/// Stored entry leaves belong to that referent, even when a nested reference
/// gives one the same empty structural owner path. This type/role query grants
/// no contents fact, inner descriptor permission or lifetime extension.
pub(crate) fn is_borrowed_parameter_root(
    program: &dyn validation::CheckingInput,
    root: facts::PlaceRoot,
) -> bool {
    let Some(symbol) = root.as_symbol() else {
        return false;
    };
    if !symbol.is_valid() || program.symbols.get(symbol).kind != symbols::SymbolKind::Parameter {
        return false;
    }
    let Some(state) = program.state_by_symbol(program.symbols.get(symbol).parent) else {
        return false;
    };
    program
        .state_parameter_by_symbol(state, symbol)
        .is_some_and(|parameter| {
            !parameter.is_const
                && !parameter.relevance.is_erased()
                && validation::reference_shell(program, parameter.type_reference).is_some()
        })
}

/// Paired coordinates produced by the one reference-formation judgment. The
/// representation address is distinct from its full retained byte footprint.
/// Both enter the same alias rebase; no consumer reconstructs admission.
pub(crate) struct BorrowAccessGeometry {
    pub(crate) place: BorrowAccessPlace,
    pub(crate) representation_address: Option<BorrowAccessPlace>,
}

impl BorrowAccessGeometry {
    pub(crate) fn declared(place: BorrowAccessPlace) -> Self {
        Self {
            place,
            representation_address: None,
        }
    }

    /// Primitive image formation can supply only read access, independently
    /// of the receiving reference annotation. Both direct bindings and carried
    /// constructor leaves use this same attenuation; transport preserves the
    /// resulting loan kind rather than upgrading it from the destination.
    pub(crate) fn retained_access(
        &self,
        program: &dyn validation::CheckingInput,
        requested: &checked_trees::BorrowAccessKind,
    ) -> checked_trees::BorrowAccessKind {
        if is_image_storage_root(program, self.place.root) {
            checked_trees::BorrowAccessKind::Read
        } else {
            requested.clone()
        }
    }
}

/// An exact primitive image root outlives every activation and contains only
/// immutable bytes, with no interior descriptor or owned cleanup dependency.
/// This classifies the already captured root, never a local's initializer or
/// another equal-valued literal. Access still needs its actual loan occurrence.
pub(crate) fn is_image_storage_root(
    program: &dyn validation::CheckingInput,
    root: facts::PlaceRoot,
) -> bool {
    let facts::PlaceRoot::Expression(expression) = root else {
        return false;
    };
    program.expression_table.expression_is_valid(expression)
        && matches!(
            program.expression_table.expression(expression),
            ExpressionNode::String(_)
        )
}

pub(crate) fn borrow_access_place(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    machine_symbol: SymbolHandle,
) -> Option<BorrowAccessPlace> {
    borrow_access_geometry(
        program,
        state_symbol,
        statement_index,
        expression,
        machine_symbol,
    )
    .map(|geometry| geometry.place)
}

pub(crate) fn borrow_access_geometry(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    machine_symbol: SymbolHandle,
) -> Option<BorrowAccessGeometry> {
    // Literal image storage is its own established immutable source. Record
    // that source at formation so later stored descriptor loads retain a real
    // occurrence rather than recovering an initializer or granting an address
    // from a lifetime profile. A literal can never supply mutable authority.
    if is_image_storage_root(program, facts::PlaceRoot::Expression(expression)) {
        // Literal establishment intrinsically supplies shared image storage.
        // A literal has no declaration-backed type identity for the place
        // type query to recover; the receiving type cannot create its grant.
        return Some(BorrowAccessGeometry::declared(BorrowAccessPlace {
            incoming_reference: Default::default(),
            root: facts::PlaceRoot::Expression(expression),
            segments: Vec::new(),
        }));
    }
    // An indexed recast can cover more than its selected element. Its one
    // formation judgment produces the complete footprint together with the
    // address; later loan correspondence consumes this admitted geometry.
    let mut viewed = expression;
    viewed = program.expression_table.peeled_expression_handle(viewed);
    if let Some(cast) = program.expression_table.cast_node(viewed)
        && cast.form.is_recast()
    {
        let mut source = cast.value;
        source = program.expression_table.peeled_expression_handle(source);
        if matches!(
            program.expression_table.expression(source),
            ExpressionNode::Indexed(_)
        ) {
            let machine = program.machine_by_symbol(machine_symbol)?;
            let state = program.state_by_symbol(state_symbol)?;
            let access = cast.form.recast_access()?;
            if let Some(footprint) = validation::validate_literal_indexed_recast_footprint(
                program,
                machine,
                state,
                statement_index,
                expression,
                cast.target_type,
                access,
            ) {
                let mut place = borrow_access_place(
                    program,
                    state_symbol,
                    statement_index,
                    footprint.collection(),
                    machine_symbol,
                )?;
                let mut address = place.clone();
                address.segments.push(facts::PlaceSegment::FixedIndex {
                    index: footprint.start(),
                });
                place.segments.push(facts::PlaceSegment::FixedRange {
                    start: footprint.start(),
                    end: footprint.end(),
                });
                return Some(BorrowAccessGeometry {
                    place,
                    representation_address: Some(address),
                });
            }
            let source = validation::validate_runtime_indexed_recast_source(
                program,
                machine,
                state,
                statement_index,
                expression,
                cast.target_type,
                access,
            )?;
            return borrow_access_place(
                program,
                state_symbol,
                statement_index,
                source,
                machine_symbol,
            )
            .map(BorrowAccessGeometry::declared);
        }
    }
    // Formation, flow and returned-value capture share one address algebra.
    // In particular, a computed backing root is not absence of a loan.
    let place = validation::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        expression,
    )?;
    Some(BorrowAccessGeometry::declared(BorrowAccessPlace {
        incoming_reference: Default::default(),
        root: place.root,
        segments: place.segments,
    }))
}
