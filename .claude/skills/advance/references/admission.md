# Mechanical admission for an advance iteration

Use `sh .claude/skills/advance/scripts/verify.sh` from a trusted checkout.
The assigner owns the manifest, accepted-path ledger, and gate output. Workers
report commits; they do not author their own successful gate records. This is
the same procedure in Claude Code and Codex, independent of their agent APIs.

1. Pin the fetched base by full SHA. Assign exact files or directory prefixes
   in a session-local manifest, one `worker path` pair per line. Do not include
   trailing slashes. A path covers itself and its descendants. Shared boards,
   rosters, and instruction files belong to one worker or the integrator.
   Validate the complete manifest before spawning:

   ```sh
   checker=/c/omega-main/.claude/skills/advance/scripts/verify.sh
   session=/c/omega-session
   mkdir -p "$session"
   printf 'parser omega-rust/psi/pipeline/tokens-to-syntax-trees\n' > "$session/lanes"
   printf 'runtime omega-rust/omega/backend/runtime\n' >> "$session/lanes"
   sh "$checker" lanes "$session/lanes" || exit 1
   : > "$session/accepted-paths" # initialize ONCE, never between workers
   ```

   Replace example paths and owners with the actual task. Unsupported path
   characters, traversal components, empty manifests, duplicate paths, and
   ancestor/descendant overlaps fail closed. The supported repository paths
   use ASCII letters, digits, underscores, hyphens, dots, and slashes.

2. Give the worker its lane, absolute worktree and scratch paths, pinned base,
   acceptance probe, and instruction to checkpoint without pushing/rebasing.
   Only one worker builds or gates at a time. Wait for its foreground command
   to finish even when the tool yields a process/session handle.

3. At a clean checkpoint, run the baseline gates through the checker in that
   worktree. Use a fresh absolute output directory outside every checkout:

   ```sh
   worker=/c/omega-parser
   revision=$(git -C "$worker" rev-parse HEAD) || exit 1
   sh "$checker" gates "$worker" "$session/parser-gates" || exit 1
   ```

   The checker records all five command exits even when a gate fails, preserves
   full logs, and returns nonzero unless all five pass. Only success writes
   `GREEN`. No truncating pipeline or completion sentinel means success.
   Keep the checkout idle during this command. Fix failures and checkpoint
   again; use fresh output after every changed SHA. Additional acceptance probes
   still have to pass and belong in the report.

4. Before cherry-picking, require both checks; only then extend the ledger:

   ```sh
   sh "$checker" commit "$session/lanes" parser "$worker" "$base" "$revision" \
     "$session/accepted-paths" > "$session/parser-paths" || exit 1
   sh "$checker" green "$worker" "$revision" "$session/parser-gates" || exit 1
   cat "$session/parser-paths" >> "$session/accepted-paths"
   ```

   `base` is the full SHA saved before dispatch, never a moving branch name.
   The checker requires HEAD to equal the supplied full SHA and no tracked or
   untracked changes. It examines every commit after base, including edits
   later reverted and both ends of a rename. Merges, out-of-lane paths, and any
   overlap with previously accepted paths are refused. An integrator changing
   a shared file must account for that work separately, not clear the ledger.

5. Gate the clean integrated SHA with `gates` and verify it with `green` just
   before landing. Cherry-picking or rebasing creates a new SHA, so worker
   evidence cannot substitute for integration evidence. If main advances,
   rebase and gate again. If any command fails, do not run the dependent
   cherry-pick, fast-forward, or push.

The checker is a local guard against workflow mistakes, not a security boundary
against an agent allowed to rewrite it or its evidence. Keep the checked script
and evidence under the assigner's control. When changing the checker itself,
review that diff and run `sh scripts/verify-test.sh` before trusting it.
