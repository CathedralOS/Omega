use selected_instructions::VirtualRegisterId;

/// Early-clobber validation needs both direct, directed ties and the number of
/// tied early definitions connected through any chain of ties. Resolve the
/// chains once and use compact indices for the registers that participate.
pub(crate) struct TiedRegisters {
    edges: Vec<(VirtualRegisterId, VirtualRegisterId)>,
    registers: Vec<VirtualRegisterId>,
    components: Vec<Component>,
}

struct Component {
    root: usize,
    size: usize,
    early_definitions: usize,
}

impl TiedRegisters {
    pub(crate) fn new(
        mut edges: Vec<(VirtualRegisterId, VirtualRegisterId)>,
        early_definitions: impl IntoIterator<Item = VirtualRegisterId>,
    ) -> Self {
        edges.sort_unstable();
        edges.dedup();
        let mut registers: Vec<_> = edges
            .iter()
            .flat_map(|&(source, definition)| [source, definition])
            .collect();
        registers.sort_unstable();
        registers.dedup();
        let mut components: Vec<_> = (0..registers.len())
            .map(|root| Component {
                root,
                size: 1,
                early_definitions: 0,
            })
            .collect();
        for (source, definition) in &edges {
            let source = registers.binary_search(source).unwrap();
            let definition = registers.binary_search(definition).unwrap();
            let mut left = component_root(&mut components, source);
            let mut right = component_root(&mut components, definition);
            if left == right {
                continue;
            }
            if components[left].size < components[right].size {
                std::mem::swap(&mut left, &mut right);
            }
            components[right].root = left;
            components[left].size += components[right].size;
        }
        for index in 0..components.len() {
            component_root(&mut components, index);
        }
        for definition in early_definitions {
            if let Ok(index) = registers.binary_search(&definition) {
                let root = components[index].root;
                components[root].early_definitions += 1;
            }
        }
        Self {
            edges,
            registers,
            components,
        }
    }

    pub(crate) fn contains(&self, register: VirtualRegisterId) -> bool {
        self.registers.binary_search(&register).is_ok()
    }

    pub(crate) fn is_only_early_definition(
        &self,
        source: VirtualRegisterId,
        definition: VirtualRegisterId,
    ) -> bool {
        self.edges.binary_search(&(source, definition)).is_ok()
            && self
                .registers
                .binary_search(&definition)
                .is_ok_and(|index| {
                    self.components[self.components[index].root].early_definitions == 1
                })
    }
}

fn component_root(components: &mut [Component], mut index: usize) -> usize {
    while index != components[index].root {
        let parent = components[index].root;
        components[index].root = components[parent].root;
        index = components[index].root;
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_ties_and_component_counts_are_independent() {
        let reg = VirtualRegisterId;
        let edges: Vec<_> = (1..=128)
            .rev()
            .map(|index| (reg(index), reg(index - 1)))
            .chain([(reg(0), reg(128)), (reg(u32::MAX), reg(200))])
            .collect();
        let ties = TiedRegisters::new(edges.clone(), [reg(0), reg(200)]);
        assert!(ties.is_only_early_definition(reg(1), reg(0)));
        assert!(ties.is_only_early_definition(reg(u32::MAX), reg(200)));
        assert!(!ties.is_only_early_definition(reg(128), reg(0)));
        assert!(!ties.is_only_early_definition(reg(0), reg(1)));
        assert!(ties.contains(reg(64)));
        assert!(!ties.contains(reg(199)));

        let ties = TiedRegisters::new(edges, [reg(0), reg(127), reg(200)]);
        assert!(!ties.is_only_early_definition(reg(1), reg(0)));
        assert!(!ties.is_only_early_definition(reg(128), reg(127)));
        assert!(ties.is_only_early_definition(reg(u32::MAX), reg(200)));
    }
}
