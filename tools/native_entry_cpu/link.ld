ENTRY(_start)
SECTIONS {
 . = 0x100000;
 .boot : { *(.boot) }
 . = 0x110000;
 .bss (NOLOAD) : { *(.bss) }
 harness_end = .;
 . = PAYLOAD_ADDRESS;
 .payload : { *(.payload) }
 image_end = .;
 memory_end = .;
 /DISCARD/ : { *(.eh_frame) *(.comment) }
}
ASSERT(harness_end < 0x200000, "Harness storage overlaps payload range")
