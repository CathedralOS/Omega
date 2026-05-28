# Omega Architecture

These notes describe compiler structure rather than language syntax.

The language guide answers "what does Omega mean?" Architecture docs answer
"which pipeline stage owns which meaning, and how does that meaning change as it
lowers?"

## Documents

- [Repository Layout](repository_layout.md): workspace/folder shape and placement rules.
- [Pipeline Overview](pipeline/README.md): durable pipeline stages and the normalized questions every stage should answer.
- [Semantic Spine](pipeline/semantic_spine.md): shared compiler nouns such as places, values, facts, loans, moves, drops, calls, transitions, effects, and boundary edges.
- [Current Pipeline Stages](pipeline/stages.md): stage-by-stage ownership of those nouns.

## Architecture Rule

The same semantic concepts should be visible across the compiler, but they should
not be forced into one mega-IR. Each stage should use the form that matches its
resolution level while preserving stable links back to the shared semantic spine.
