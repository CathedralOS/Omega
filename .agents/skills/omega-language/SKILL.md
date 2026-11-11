---
name: omega-language
description: How to write, explain, or debug Omega (.omg) code without guessing. Use before writing any Omega example, sample, fixture, or doc snippet, or when answering what Omega syntax or semantics is. Routes to the repository files that define the language and to the commands that prove an example compiles.
---

# Omega Language

Omega is new and changes often, so nothing you remember about it is reliable,
and anything copied into this skill would go stale. This skill holds no syntax.
It tells you where the current answer lives and how to prove an example is
real. Every Omega form you write must be looked up in a file and checked by the
compiler.

Omega is not Rust with different keywords. Code that looks plausible because it
resembles Rust, Zig, Ada, or Dafny is the usual way Omega examples come out
wrong.

## The loop for every example

1. **Look up the form.** Open the guide chapter or spec file for the topic (see
   below) and read its current spelling. Do not rely on an earlier read in
   another session; the files move.
2. **Find a passing instance.** Search `tests/omega/pass/` and `samples/` for a
   fixture that uses the form, and confirm its row in
   `tests/omega/corpus_outcomes.txt` is passing. Copy from that file verbatim
   and adapt as little as possible.
3. **Prove it.** Run the example through the current compiler, built from the
   current `main` (see `AGENTS.md` Commands; `mbx` is mandatory). Command
   spellings are defined in `wiki/spec/build/command_line.md`; use them as
   written there. `--check` passing is not enough for runnable code: run it
   (`omega run --both`) and inspect the actual output.
4. **Cite it.** When showing an example to a person, name the file you took the
   form from and say whether you ran it. If you could not run it, say so.

If no passing instance exists, or the wiki and the compiler disagree, say that
plainly. Do not invent a middle spelling.

## Which source wins

1. **What the language is:** `wiki/spec/`. `wiki/language_guide/` is a short
   tutorial that shows the common forms and links to the rules; where it is
   silent, the specification decides. Start at `wiki/README.md`.
2. **What works today:** the compiler built from current `main`.
3. **Evidence, not authority:** `tests/omega/`, `samples/`, `source/library/`.
   These lag the wiki during migrations. Before copying a spelling, check
   `samples/README.md` and `TASKS.md` for an active migration that retires it,
   and check the spelling against the wiki.
4. **Not current rules:** `wiki/proposals/`, `wiki/drafts/`, `OWNER_QUESTIONS.md`.

## Where to look

- **Topic → guide chapter:** list `wiki/language_guide/` and open the chapter
  whose file name matches the topic (`language_guide.md` is the index). Chapter
  names change, so list the directory instead of assuming a file name.
- **Precise semantics:** `wiki/spec/language/` (one file per subject: machines,
  ownership, numeric values, patterns, domains, termination, …),
  `wiki/spec/build/` (`build.omg`, entry roots, providers, command line),
  `wiki/spec/proofs/`, `wiki/spec/layouts/`.
- **Bundled library APIs:** `source/library/` (`name.omg` or `name/mod.omg`).
- **Whole programs:** `samples/` (each `main.omg` header states its expected
  exit or output) and `samples/README.md`.
- **Focused behaviors and their diagnostics:** `tests/omega/{pass,fail,run}/`,
  with `tests/omega/README.md` and `tests/omega/corpus_outcomes.txt`.
- **Known compiler gaps:** pinned regressions
  (`mbx xtask corpus --list-pinned-regressions`) and `TASKS.md`. Search there
  for the diagnostic you hit before working around it.
