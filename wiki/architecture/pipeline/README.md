# Pipeline Architecture

Omega's compiler pipeline should be a sequence of durable representation
boundaries, not a pile of ad hoc helper structs.

Each stage should answer the same normalized questions at its own resolution
level:

- What are the relevant places?
- What values exist, and where do they come from?
- What facts are known at this point?
- What loans are active?
- What moves happen?
- What drops are required or scheduled?
- What calls happen?
- What transitions happen?
- What effects are introduced directly or transitively?
- What boundary edges are crossed?

The answer changes by stage. Source-shaped IR can only say "this syntax looks
like a place." Checked IR can say "this place overlaps this loan." Backend IR
can say "this place is stack slot plus offset." The vocabulary is shared; the
data is stage-specific.

## Current Documents

- [Semantic Spine](semantic_spine.md)
- [Current Pipeline Stages](stages.md)

## Normalized Stage Template

Every pipeline stage doc should answer:

- Input representation.
- Output representation.
- Primary responsibility.
- What semantic nouns become more resolved here.
- What semantic nouns must not be invented here.
- Ownership/proof questions this stage can answer.
- Ownership/proof questions this stage must defer.
- Artifact or diagnostics this stage should emit.
- Known tech debt and next cleanup target.
