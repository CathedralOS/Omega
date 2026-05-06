# Point And Click Sample

This sample is intentionally pseudocode. Its purpose is to exercise the Omega machine model against a small game-like scenario.

What it is trying to prove:

- `main` clearly owns runtime concerns like window creation and the game loop
- `state entry` is the implicit entry point for a machine named `main`
- process exit status is owned data, not a special return value
- a dedicated room manager owns active-room selection and room dispatch
- each room machine owns its own state and interaction logic
- control flow is explicit as state-to-state handoff, not disguised function returns
- platform calls are isolated behind a stub-friendly machine boundary
- debugger position should be derived from active state and source location
- cross-machine data flow can happen through `mut` context instead of transition return values
- transitions are graph edges, not executable function bodies
- transition order is the branching model

Current sketch:

- [main.omg](/Users/zcanann/Documents/projects/Omega/samples/point_and_click/main.omg:1) is the sample entry point
- `main` owns `Window::create(...)`, frame polling, presentation sequencing, and the top-level handoff states
- `main.return_code` models OS process result as mutable owned data, then `ExitProcess` hands it to the platform
- `DesktopPlatform` is a platform boundary machine
- `Game` owns frame-level composition and a persistent `MainView`
- `RoomManager` owns active-room selection, inventory, and room dispatch
- `FoyerRoom` and `CellarRoom` keep room-specific state local
- `state` is executable code, calls mutate explicit `mut` context, and trailing `-> target` lines declare exits
- state-local transition order is the branch table; a bare `-> target` is unconditional
- `-> self;` re-enters the current state
- trailing bare `->` marks terminal completion for called helper states

Sample layout:

- `main.omg`: top-level machine wiring
- `data/`: shared data structures and enums
- `game/`: frame-level game orchestration
- `rooms/`: room ownership, dispatch, and room-specific behavior
- `platform/`: explicit platform boundary machines

Questions this sample should help answer next:

- Should `query` be able to write into arbitrary output structs, or should views be preallocated owned buffers only?
- How strict should cross-machine `mut` access rules be?
- Should event-driven transitions suspend a state, or should they always flow into an explicit waiting state?
- Should called states also be branch-free, or is branch-free execution only required for scheduled states?
