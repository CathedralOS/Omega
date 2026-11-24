# Psi and native boundary packet

This packet gives each claim in [the Psi/native boundary note](../../boundaries/psi_and_native.md) a disposition. The note was written at `4f3c255d`. Every code check below was made at the common baseline `24ad2700685e6cc6062026c87ed5fd24ed415265` unless a row names another revision. It joins the dispositions already in the [Psi and native ledger](../07-13-psi-and-native.md) rather than repeating them.

Outcomes: **fixed** (repaired, here or in a named PR), **intended** (the code does what its contract says), **duplicate** (another ledger row already settles it), **feature gap** (missing behavior the spec or a task tracks), **reproduced** (a fixture shows the defect), **still open** (not settled; the next check is named).

Status: in progress.

| # | Note section | Claim | Code path at baseline | Contract, fixture or trace | Outcome |
| --- | --- | --- | --- | --- | --- |
| H1.1 | Handoff 1, "What is checked", saved bytes, last sentence | The certificate reader accepts proof-row tags 0–27, the writer emits 0–32. | `psi/artifact/callable-encoding/src/certificates.rs:677-704` has reader arms for tags 28–32. | Ledger row "08, certificate read/write coverage" names #5181. | duplicate (fixed in #5181) |
