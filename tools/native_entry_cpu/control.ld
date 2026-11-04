ENTRY(control)
SECTIONS { . = 0x200000; .text : { *(.text) } /DISCARD/ : { *(.eh_frame) *(.comment) } }
