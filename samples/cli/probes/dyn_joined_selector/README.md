# dyn_joined_selector

Per-edge `dyn` selectors converging on one state: `transition main.double { (true) -> run(main, Double as dyn Step<Counter>) _ -> run(main, Increment as dyn Step<Counter>) }` — each edge carries its own selected implementation, the join invents no representative; `Step::apply(step, &mut counter)` dispatches through the dyn param.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
