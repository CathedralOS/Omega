# OMEGA-COMPILER-TAPE-BINDING — re-verification ledger

Board row: `TASKS.md` `**OMEGA-COMPILER-TAPE-BINDING.**` (:13840) — mined
candidate, dependency gate recorded at `f3d0d1748e`, re-verified at
`94e764a6da` / `c0a3f3ac34` / `832c55e69b` / `69cde2eb78` /
`53817f8759`. Re-verified on this host (linux x86-64) at `75650d2e94`
by w10 zergling Z26 (claim `d57f2ab1`, draft path only).

## Nothing to bind — confirmed

The clause is CHAIN-MANIFEST's remaining-work bullet: "The `omega0` and
`omega` compiler tapes, which OMEGA-C has yet to produce"
(TASKS_BOOTSTRAP.md:298, unchanged). `bootstrap/` still carries exactly
two tapes — `1_beta/beta_compiler_bytecode.tape` and
`2_gamma/gamma_evaluator_bytecode.tape` — plus the `5_omega/` epsilon
sources (`alpha_tape.epsilon`, `lexer.epsilon`, `parser.epsilon`,
`scalar_compilation.epsilon`, `request_and_utf8.epsilon`,
`outcome.epsilon`, `representations.epsilon`,
`lexical_classification.epsilon`, `omega_compiler.epsilon.sources`). A
whole-tree `*tape*` sweep at `75650d2e94` adds only
`tests/epsilon/interpreted-omega-experiment/customers/omega_alpha_tape/`
— an experiment customer entry (`main.epsilon`), not a produced tape.
No `omega0`/`omega` tape exists anywhere, so no binding surface has a
referent: `tools/bootstrap/omega/compiler_env.sh` still pins only the D
closure (`OMEGA_COMPILER_{MANIFEST,PACKED}_*`) and the gate-local
customer entries (`OMEGA_{PARSER,OUTCOME,REQUEST,EXECUTABLE_*}_*`) —
the `*_env.sh` pin pattern extends only once each tape exists.

## Producing rung — still gated

OMEGA-D's acceptance (TASKS_BOOTSTRAP.md:236 — "interpreted D compiles
the exact Omega C closure for its ordinary `alpha_bootstrap` target and
produces `omega0_compiler_bytecode.tape`") has not occurred. Live claims
over the producing surfaces at this verification are item-level, not
dir-claims: OMEGA-D-ENTRY-ADAPTER (exp 15:43Z),
BOOTSTRAP-CHAIN-NATIVE-EXECUTION over
`wiki/drafts/bootstrap_chain_native_execution.md` (exp 20:53Z),
DELTA-COMPILER (exp 18:01Z), and BETA-PE-SEED-REFUSAL over
`tools/bootstrap/{beta,alpha/seed_env.sh}` (exp 16:12Z). Unlike prior
verifications no live claim covers `bootstrap/5_omega`,
`tools/bootstrap/omega`, `tests/bootstrap/omega-*`, or `source/omega`
directly — but the gate is the absent tape, not the fence map: the
ordered frontier is unchanged — OMEGA-D acceptance → D compiles the
`source/omega` closure → omega0 tape bound here → `omega0` self-host
compile → `omega` recompile equivalence. No independent slice exists;
the correct artifact is this ledger.
