//! Physical stack guards need the frame and arrival bridge of each handler.
//! These edges are a subset of the closed callback roster. Ordinary callback
//! references alone impose no ordering on physical planning.

/// Visit only surviving installation edges. The first list contains exactly the
/// dependencies of the root's numeric guards, in dependency order. The second
/// finishes the private roster after the root's own frame has been planned.
pub fn handler_planning_order(
    dependencies: &[Vec<usize>],
    root_handlers: &[usize],
    retained: &[usize],
) -> Result<(Vec<usize>, Vec<usize>), &'static str> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum State {
        Unseen,
        Visiting,
        Finished,
    }
    let mut states = vec![State::Unseen; dependencies.len()];
    let mut pending = Vec::new();
    let mut visit = |roots: &[usize]| {
        let mut order = Vec::new();
        for &root in roots {
            pending.push((root, 0));
            while let Some((node, next)) = pending.last_mut() {
                let node = *node;
                let state = states
                    .get_mut(node)
                    .ok_or("installed handler is outside the callback roster")?;
                if *state == State::Finished {
                    pending.pop();
                    continue;
                }
                *state = State::Visiting;
                if let Some(&child) = dependencies[node].get(*next) {
                    *next += 1;
                    match states.get(child) {
                        None => return Err("installed handler is outside the callback roster"),
                        Some(State::Visiting) => {
                            return Err(
                                "installer and handler have a cyclic executable resource dependency",
                            );
                        }
                        Some(State::Unseen) => pending.push((child, 0)),
                        Some(State::Finished) => {}
                    }
                } else {
                    states[node] = State::Finished;
                    order.push(node);
                    pending.pop();
                }
            }
        }
        Ok(order)
    };
    let before_root = visit(root_handlers)?;
    Ok((before_root, visit(retained)?))
}

#[cfg(test)]
mod tests {
    use super::handler_planning_order;

    #[test]
    fn shared_handlers_precede_each_installer_and_ordinary_callbacks_follow_root() {
        let edges = [vec![2], vec![2], vec![], vec![1], vec![4]];
        assert_eq!(
            handler_planning_order(&edges, &[0], &[0, 3, 1, 2]),
            Ok((vec![2, 0], vec![1, 3]))
        );
        assert_eq!(
            handler_planning_order(&edges, &[], &[4]).unwrap_err(),
            "installer and handler have a cyclic executable resource dependency"
        );
        assert!(handler_planning_order(&edges, &[5], &[]).is_err());
    }

    #[test]
    fn nested_installations_use_bounded_heap_work() {
        let count = 50_000;
        let mut edges = (1..count).map(|child| vec![child]).collect::<Vec<_>>();
        edges.push(vec![]);
        let (order, rest) = handler_planning_order(&edges, &[0], &[]).unwrap();
        assert!(order.into_iter().eq((0..count).rev()));
        assert!(rest.is_empty());
    }
}
