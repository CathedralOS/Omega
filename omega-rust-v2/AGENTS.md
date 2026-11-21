# Working on Omega V2

Use `C:\Development\Omega-V2` for V2 commands and file access. Use `mbx` for Rust builds, runs, tests and Clippy against the V2 Cargo workspace. If the junction is missing or broken, ask for its target.

## References and decisions

Use the repository’s shared `wiki/spec/` for language and portable product requirements. Use V2’s `PLAN.md`, layer contracts, source, and tests to determine which V2 layer owns a decision and what its consumer receives.

You may read V1 specifications, documentation, source, tests, samples, and Git history to understand prior behavior or find a working implementation. When using V1 as precedent, identify the governing shared-spec rule and check the behavior at the V2 handoff.

Record unresolved language or architecture decisions in the repository-root `OWNER_QUESTIONS.md`. Follow the queue’s admission rules, cite the governing sections, and search for an existing question first. Keep outstanding questions open until the owner rules. Changes to the shared specification require explicit authorization.

## Skills

Invoke a skill only when the user explicitly requests it in chat.
