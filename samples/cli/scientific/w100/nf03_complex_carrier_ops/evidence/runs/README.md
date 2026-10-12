# Retained qualification attempts

All logs contain exact argv and full emitted output; JSON records exit status
and elapsed time. Complete source snapshots keep `.omg.txt` suffixes to prevent
sample discovery or package loading from treating historical failures as new
programs. Rename that suffix to `.omg` in a scratch directory to reproduce.
Some snapshot Build paths name this executor; replace only harness metadata
paths when reproducing elsewhere. Owned implementation/main source is preserved.

- sandbox-preflight: old bundled binary, cache custody refusal.
- old-unsandboxed-preflight: old binary, pre-check Build syntax refusal.
- current-sandbox: fresh baseline compiler, cache custody refusal.
- current-constructor-failure: first fresh unsandboxed attempt; phasor constructor
  syntax failure, stopped before repeating it across all projects.
- current-corrected: final owned source, all route commands completed. Both valid
  samples and the alias control are blocked by 240 core float-provider binding
  errors. This does not qualify numerical execution or intended rejection.

Rust build log belongs to baseline 9d4451128dd05885e7b9c591796b2760189851db;
source-only candidate changes do not change that compiler. Baseline attribution
uses a minimal program without NF03 declarations or imports.
