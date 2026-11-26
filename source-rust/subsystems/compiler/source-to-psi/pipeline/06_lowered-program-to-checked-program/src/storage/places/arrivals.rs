//! The declarations and edge arguments of one machine's structural block inputs.
//! A source lineage identifies an edge operand. Its contents and any captured
//! backing are established at the actual edge whenever a storage query follows it.

use super::*;
use std::cell::OnceCell;

#[derive(Default)]
pub(super) struct Arrival {
    borrowed: OnceCell<bool>,
    sources: OnceCell<Vec<(BlockId, Lineage)>>,
}

impl Storage<'_> {
    pub(super) fn is_borrowed_arrival(&self, root: PlaceId, concerns: Construct) -> Checked<bool> {
        let StructuralPlaceKind::BlockParameter { block, position } =
            self.root_kind(root, concerns)?
        else {
            return Ok(false);
        };
        let Some(target) = self.lowered.blocks.get(index(block.get())) else {
            return Ok(false);
        };
        let position = position as usize;
        if target
            .structural_parameters
            .get(position)
            .is_none_or(|parameter| parameter.place != root)
        {
            return Ok(false);
        }
        let slot = &self.arrivals[index(block.get())][position];
        Ok(*slot.borrowed.get_or_init(|| {
            terminal_semantics::arrival_parameter(self.lowered, root).is_some_and(|parameter| {
                parameter.access != StructuralAccess::Owned
                    && !terminal_semantics::descriptor_results::is_descriptor(
                        self.types.iter(),
                        parameter.structural_type,
                    )
            })
        }))
    }

    /// Visit every incoming binding of one block input in checker order.
    /// Retain its argument lineages only after each edge and its requested
    /// observation have been checked.
    pub(super) fn visit_incoming_storage(
        &self,
        target: &Block,
        position: usize,
        concerns: Construct,
        mut visit: impl FnMut(BlockId, &Lineage) -> Checked<()>,
    ) -> Checked<()> {
        let slot = &self.arrivals[index(target.id.get())][position].sources;
        if let Some(sources) = slot.get() {
            for (predecessor, source) in sources {
                visit(*predecessor, source)?;
            }
            return Ok(());
        }
        let parameter = &target.structural_parameters[position];
        let mut predecessors = self.graph.predecessors(target.id)?.to_vec();
        predecessors.sort_unstable();
        predecessors.dedup();
        let mut sources = Vec::new();
        for predecessor in predecessors {
            let terminator = &self.block(predecessor)?.terminator;
            let successors = terminator.successor_edges();
            let bindings = terminator.successor_block_arguments();
            if successors.len() != bindings.len() {
                return Err(self.foreign(concerns, "an edge has no complete binding roster"));
            }
            for ((edge, into), arguments) in successors.into_iter().zip(bindings) {
                if into != target.id {
                    continue;
                }
                if arguments.len() != target.structural_parameters.len() {
                    return Err(self.foreign(
                        Construct::Edge(edge),
                        "structural edge arguments differ from the target telescope",
                    ));
                }
                let argument = &arguments[position];
                if !terminal_psi::structural_access_covers(argument.access, parameter.access) {
                    return Err(self.foreign(
                        Construct::Edge(edge),
                        "a structural edge argument has insufficient access",
                    ));
                }
                let source = self.argument(argument, Construct::Edge(edge))?;
                if !terminal_semantics::successor_argument_carrier_matches(
                    self.types,
                    self.root_type(argument.place, Construct::Edge(edge))?,
                    argument,
                    parameter,
                ) {
                    return Err(self.foreign(
                        Construct::Edge(edge),
                        "a structural edge argument has another type",
                    ));
                }
                visit(predecessor, &source)?;
                sources.push((predecessor, source));
            }
        }
        if sources.is_empty() {
            return Err(self.foreign(concerns, "a block formal has no incoming binding"));
        }
        slot.get_or_init(|| sources);
        Ok(())
    }
}
