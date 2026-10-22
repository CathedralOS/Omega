# Repository techniques for source ingress — 2026-10-21

Read-only follow-up to the source assembly experiments. No implementation or timing
experiment performed. Omega inspected at published revision
50ace1653635745e0227821341117f280c25779d using git show; the primary checkout is
older, so working-tree source excerpts were not used to establish current gaps.
Prior research: source_assembly_2026_10_21/ownership/research.md,
probe_storage/research.md, and the import_custody experiment.

Search batch, exact queries:
- site.github.com/rust-lang/rust compiler source_map file_loader SourceFile
- site:github.com/llvm/llvm-project clang FileManager UniqueRealFiles FileEntryRef
- site:github.com/rust-lang/rust-analyzer VfsPath FileId interner

Search results were discovery only. Read actual implementations at these URLs:
- https://raw.githubusercontent.com/rust-lang/rust-analyzer/master/crates/vfs/src/path_interner.rs
  PathInterner stores paths once in IndexSet and exposes compact FileId values.
  Omega queue still stores (PathBuf, DependencyScope) in a map and pending deque.
  Candidate: source-view-local path handles, separate checked-instance identities,
  root/package metadata carried once; dense queue membership after admission.
- https://raw.githubusercontent.com/rust-lang/rust-analyzer/master/crates/hir-def/src/item_tree.rs
  ItemTree is a compact item-level representation used by name resolution and
  incremental invalidation. Current implementation has crate input too; do not
  claim all modern ItemTree computation is wholly crate-independent from its header.
  Omega discovery still scans every root item for Item::Use. Candidate: parser
  publishes import occurrence handles/ordinals in the existing parsed-source batch.
  This is an adaptation, not a claim rust-analyzer emits this list from its parser.
- https://raw.githubusercontent.com/ziglang/zig/master/lib/std/zig/Ast.zig
  Tokens and nodes use MultiArrayList; token tags and offsets are separate arrays.
  Parse preallocates from empirical source/token/node ratios. Omega uses Vec<Token>
  with kind, lexeme and span together. Candidates: split frequently scanned tags
  from payload, and measured capacity planning. Do not transplant Zig's ratios.
- https://raw.githubusercontent.com/rust-analyzer/rowan/master/src/green/node.rs
  Immutable GreenNode stores ThinArc-backed children and rebuilds edited parents
  while retaining other children. Omega shared checkpoint uses Arc::unwrap_or_clone
  on SourceStorage. Candidate: immutable arena pages plus child-owned append storage,
  preserving generation identities; do not transplant per-node reference counting.
- https://raw.githubusercontent.com/evanw/esbuild/main/internal/cache/cache_ast.go
  Parse cache compares source identity/contents and parser options, replaying messages.
  Candidate: exact source/options/version keyed immutable syntax reuse. Distinct from
  semantic or checked-package reuse; current parser source IDs require a careful
  instance binding/remapping design. More useful for repeated compilations.
- https://raw.githubusercontent.com/llvm/llvm-project/main/clang/include/clang/Basic/FileManager.h
- https://raw.githubusercontent.com/llvm/llvm-project/main/clang/lib/Basic/FileManager.cpp
  Arena-owned entries, canonical-name storage, path/unique-file maps, positive and
  negative lookup caching. Reuse the identity/ownership lesson. Do not copy inode
  merging or host negative caching into Omega's captured-source authority model:
  source_file_system explicitly preserves capture namespaces and host alias freshness.

Priority: path/root identity and import occurrence lists are closest to the current
assembly layer. Shared syntax pages and compact token columns require broader storage
experiments. Capacity planning is a smaller experiment. None is a measured Omega win.
Decisive measurements: path allocations/hash bytes; non-import roots scanned; bytes
cloned per shared child; token cache traffic and whole parse time; growth events and
peak retained capacity. Keep exact outputs/diagnostics, source-view custody, build/product
scope, source occurrence ordinals, generations and native/Terminal outcomes unchanged.

Follow-up: the import-range experiment in [README.md](README.md) measures and retains a parser-produced range over existing roots. The file-identity direction remains unimplemented; queue counts are now measured, but not a handle implementation or speedup.
