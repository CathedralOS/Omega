# Atlas packet unit_018: x86-64 selected-form encoder, lines 1-7000

Status: in progress.

This packet checks each ordinary correctness observation in [record unit_018](../../records/unit_018.md) against the source at the common baseline `24ad2700685e6cc6062026c87ed5fd24ed415265`. The record covers lines 1-7000 of `X64sfe`, the x86-64 selected-form encoder at `source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding/isa-x86_64/src/selected_form_encoding.rs`. Lines 7001-21917 belong to the gap packets `unit_018_gap_004` to `unit_018_gap_006` and are not reviewed here.

Between the survey commit `4f3c255d` and the baseline, only [#5392](https://github.com/CathedralOS/Omega/pull/5392) changed this file, and only at lines 12825-15146. Every line cited below is therefore the same at both revisions.

## Receipts

### 1. `floating_control` accepts only the shortest displacement

- **Original:** record unit_018 § floating_control: "the decoder requires disp8 in 1..=127 and disp32 >= 128 (4731-4732), so only the shortest displacement form is accepted."
- **Current source:** `X64sfe:4716-4737` (`floating_control::decode`), `:4658-4664` (`request`), `:4677-4683` (`encode_*`).
- **Contract:** `request` admits any multiple of 8 in `0..=i32::MAX` at a boundary slot. The decoder must accept the encoder's spelling of each such value.
- **Trace:** The record left out the first decoder arm. `(0, [])` (`:4728`) accepts `[rsp]` with no displacement byte for displacement 0, so 0, 1..=127 and 128.. each have exactly one accepted spelling. The test `floating_controls_bind_direction_slot_displacement_and_every_encoded_bit` (`:4766-4851`) encodes 0, 8, 120, 128, 4096 and 2147483640, round-trips each, rejects every bit flip and an extra byte, and refuses -8, 4 and 2147483648.
- **Outcome:** intended. The record is partly right: one canonical spelling per displacement, including 0.
