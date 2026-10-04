# Indexed calls through a borrowed payload

Run `omega run --both tests/omega/pass/calls/runtime_case_payload_indexed_calls_exit/main.omg`.
Both engines must exit 70. The case-selected payload crosses two named states;
receiver and explicit argument calls then select the runtime element. The
neighbor, out-of-bounds branch and empty case must each return false.

Origin replay must compose the retained Case/Field prefix with the indexed
suffix without treating endpoint typing as reference-formation permission.
