# Compile Cathedral’s interrupt acknowledgement handler

This fixture demands a native interrupt entry for Cathedral’s real PIC acknowledgement handler, including its opaque five-word argument and checked helper call. It is compiled only. It must not be invoked by the CPU harness: installation has not yet constructed a live acknowledgement argument or established its `Pending` qualification.

The source comes from the frozen `timer-borrowed-gate-valid-policy` probe. Its entry policy explicitly permits instruction-pointer and stack-pointer use while requiring their restoration. Those policy corrections distinguish it from the earlier frozen probe; the handler, PIC operations and fact declarations are retained unchanged. The root Build path resolves the same standard library.

This checks native emission of the customer’s parameter and body. It does not prove IDT publication, installed-root lifetime, timer delivery, acknowledgement issuance or ordinary-work wakeup. The original nested Resident-layout gap remains separate.
