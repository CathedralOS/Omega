# Source import / assembly audit

Base: 30fbbc91026a740ce3e709b51e8bd7b41a1c123e. Named layer-0 assignment, bounded to removing redundant owned-batch metadata copies.

Trace: ProjectSourceSelection -> ImportQueue -> load_imports -> SourceInput::append -> discover_package_imports/discover_imports -> SourceStorage::extend -> checkpoint -> assemble -> retain_module_import_bindings.

* Parser pages already move into the forest through adopt_page. No deep syntax-tree copy on that join.
* Import discovery visits each newly parsed batch; repeated load_imports calls do not rescan previous imports.
* The queue deduplicates by path AND dependency scope. That distinction must survive.
* Module binding already constructs a borrowed path index once, sorts import occurrences back to authored order, and joins exact checked scope.
* SourceStorage::extend consumes ParsedSources but clones each path twice (SourceMap and SourceFile) and clones each root vector before dropping its owner. Transfer the original path/root vector to SourceFile after discovery; retain the one independent path copy SourceMap needs. Keep source bytes shared by Arc.
* A moved root vector retains parser capacity slack. Measure its extent; no RSS reduction is inferred from avoided allocation requests.
* Shared checkpoint children still clone mutable SourceStorage. That clone separates child-local syntax and generated sources; it is outside this bounded change.
* Toolchain custody snapshots and ImportQueue duplicate-path ownership remain possible subsequent questions; no benefit measured for changing them in this iteration.

Acceptance: identical assembled syntax, source metadata, selected build identity, application and import bindings for the selected module/package/Build inputs, with rejected diagnostics unchanged; both consuming and shared-checkpoint paths. Measure caller-thread allocation reduction, ordinary CLI timing with frozen executables, affected package checks and relevant positive native/Terminal plus invalid import controls. Preserve all observations and land via normal protocol.
