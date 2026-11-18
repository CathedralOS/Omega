# Interactive architecture

**Stages 00–10** opens the numbered compiler at `c329436ed22`. Select a stage to see the Rust code behind it. Within a stage, select an operation to read the original design contract and its separately dated evidence. Stage 01 still lexes and parses one file at a time. [Per-file stream][current-stream] · [Compiler verification](../plans/foundation-and-backend.md#compiler-verification)

**Before migration** shows the full compiler at `3f9ce43f0c9d`. Orange arrows trace extra inputs, returns to earlier work and late code generation. Dotted gray arrows show supporting calls and data. Teal marks the checker. Choose a detour under **Trace**, then switch views to follow its repair. Each view remembers the position and zoom you left it at. [Original illustrated map](pipeline-map/original.html)

Trace **Handler round trip** to see how 03 finds the retained callbacks, how 04 measures each handler before selecting its installer’s stack guards, and how 05 checks those measurements against the finished program.

**Migration branch.** Select **01.2 Parse that file** in Stages 00–10 to see how fresh files pass straight from lexing to parsing and how package assembly reuses an exact earlier parse. Follow **02.2 Seal publication** to **03.1 Join provider selections** to see which checked body supplies each call. Stage 03 refuses a changed body. If computing a constant needed that provider, the executable records the exact plan while the source keeps its original identity. [Per-file stream][branch-stream] · [Published provider demand][branch-demand] · [Target planning][branch-target]

Follow **05.2 Check + bind installations** to see where a handler’s measured stack meets its installer’s guard. **08.2 Form objects** shows what 09 receives about the actual text and executable bodies. The root, callbacks and installed handlers now finish stage 08 together. If the last object fails, stage 09 emits no fragments. Component exports follow the same order. The [implementation checkpoint](../plans/foundation-and-backend.md#implementation-checkpoint) records each source revision and which execution routes still need verification. [Object-to-image handoff][branch-objects] · [Component exports][branch-exports]

Select **00.2 Admit requests** to see where a package permission lock enters the target. **02.2 Seal publication** and **03.1 Join provider selections** show how the chosen providers retain their source owner even after folding. At **Published products**, the host can run Psi alone or validate linked code ranges from a completed image for that exact program and target. These routes are traced at `0e5d56643021`. [Provider receipt][verified-receive] · [Package consent][verified-consent] · [Runtime image][verified-host]

**Public compiler.** The real Console sample printed identical Hello output through the official compiler and the migration build. Both reported native and Psi exit 0. [Compared builds and complete transcript](../plans/foundation-and-backend.md#compiler-verification)

**Native execution.** Follow **03.1 Join provider selections** to **Published products**. For the same program, the official and migration compilers emitted byte-identical executables on each of four targets. Both executables ran directly on Linux x86-64, Linux arm64, macOS arm64 and Windows x86-64. All eight exited 0. The earlier CLI checkpoint retains its offline macOS signature check. [Four-host results](../plans/foundation-and-backend.md#native-execution-on-four-matching-hosts) · [Native job logs](https://github.com/CathedralOS/Omega/actions/runs/44227027634) · [Earlier CLI checkpoint](../plans/foundation-and-backend.md#measured-public-cli-checkpoint)

The numbered route runs down each column, then across to the next one. Use Stage to jump to its operations, then follow the arrows to its output. Drag in any direction and scroll or pinch to zoom. The minimap shows your position.

Follow the main route from START, down each numbered run and across to the next group. Numbered stages show the
compilation order and phase frames group related work. In Before migration, dashed
connections show the observed supporting calls and returns; each shared component
appears once, including the Psi interpreter. The numbered pipeline puts compiler evaluation
inside its owning stage and ends at published products.

Drag to pan, use the zoom controls, and select a component or arrow to trace
its connections. Find moves to a component. Fit whole compiler and the minimap
locate the current view; Start returns to Build and 100% restores natural-scale
labels. Supporting arrows display their labels. Select one to inspect its endpoints and source. The same SVG and source disclosures work without
JavaScript. Each connection cites the recorded compiler source.

Historical loops, retained inputs and late code generation remain visible in Before migration. Their repairs appear in the
[extra-arrow repair list](../plans/foundation-and-backend.md#start-with-the-extra-arrows)
in the complete migration plan. Its
[handler explanation](../plans/foundation-and-backend.md#why-handlers-leave-and-return)
shows the current detour and proposed replacement. Orange at Apply frames marks
post-emission instruction rewriting; orange at Form image marks the late
entry/adapter generation included in that route.
The [original authored map](pipeline-map/original.html) remains a historical
presentation. The [reviewed pipeline account](pipeline-map/map.md) corrects
historical claims and traces current handoffs.

[branch-stream]: https://github.com/CathedralOS/Omega/blob/788ecf2489dabc436e00f4cf97dc5cf28b91604c/omega-rust/psi/compilation/pipeline/01_source-compilation/src/source_parsing.rs#L91-L133
[branch-demand]: https://github.com/CathedralOS/Omega/blob/788ecf2489dabc436e00f4cf97dc5cf28b91604c/omega-rust/omega/compilation/pipeline/02_portable-publication/src/provider_demand.rs#L73-L104
[branch-target]: https://github.com/CathedralOS/Omega/blob/788ecf2489dabc436e00f4cf97dc5cf28b91604c/omega-rust/omega/compilation/pipeline/03_native-program-planning/src/selected_execution.rs#L96-L124

[branch-objects]: https://github.com/CathedralOS/Omega/blob/80cc31a4df809971ed701b6ce5edcbaff6a86603/omega-rust/omega/compilation/native-compiler/src/realization/object_emission.rs#L304-L321
[branch-exports]: https://github.com/CathedralOS/Omega/blob/80cc31a4df809971ed701b6ce5edcbaff6a86603/omega-rust/omega/compilation/native-compiler/src/realization/component_exports.rs#L292-L315

[verified-receive]: https://github.com/CathedralOS/Omega/blob/0e5d56643021b7633553af23cfede3c93021e1d5/omega-rust/omega/compilation/products/src/checked_compilation.rs#L116-L147
[verified-consent]: https://github.com/CathedralOS/Omega/blob/0e5d56643021b7633553af23cfede3c93021e1d5/omega-rust/omega/compilation/compiler/src/checked/target_preparation.rs#L39-L68
[verified-host]: https://github.com/CathedralOS/Omega/blob/0e5d56643021b7633553af23cfede3c93021e1d5/omega-rust/omega/build-system/orchestration/src/published_terminal.rs#L45-L91

[current-stream]: https://github.com/CathedralOS/Omega/blob/c329436ed22f18b41352985cff202b46129e6538/omega-rust/psi/compilation/pipeline/01_source-compilation/src/source_parsing.rs#L97-L131
