use psi_arena::Arena;

pub(crate) fn remap_arena<Source, Target>(
    source: &Arena<Source>,
    mut remap_item: impl FnMut(Source) -> Target,
) -> Arena<Target>
where
    Source: Clone + Default,
    Target: Default,
{
    let mut target = Arena::with_capacity(source.len());

    for (_, item) in source.iter() {
        target.append(remap_item(item.clone()));
    }

    target
}

#[cfg(test)]
mod tests {
    use super::remap_arena;
    use psi_arena::Arena;

    #[test]
    fn remaps_active_items_without_losing_arena_shape() {
        let mut source = Arena::with_capacity(2);
        source.insert(1_u32);
        source.insert(2_u32);

        let target = remap_arena(&source, |value| value.to_string());

        assert_eq!(target.len(), 2);
        assert_eq!(
            target
                .iter()
                .map(|(_, value)| value.as_str())
                .collect::<Vec<_>>(),
            ["1", "2"]
        );
    }
}
