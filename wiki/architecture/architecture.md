# Omega Architecture

These notes describe compiler structure rather than language syntax.

The language guide answers "what does Omega mean?" Architecture docs answer
"which pipeline stage owns which meaning, and how does that meaning change as it
lowers?"

## Documents

- [Repository Layout](repository_layout.md): workspace/folder shape and placement rules.
- [Pipeline Architecture](pipeline/pipeline.md): semantic spine, durable stages, and the normalized questions every stage should answer.
- [Codegen Representation Cleanup](codegen_representation_cleanup.md): standing plan to remove re-declared representations and annotation-only stages so the backend obeys the Architecture Rule below.

## Architecture Rule

The same semantic concepts should be visible across the compiler, but they should
not be forced into one mega-IR. Each stage should use the form that matches its
resolution level while preserving stable links back to the shared semantic spine.
