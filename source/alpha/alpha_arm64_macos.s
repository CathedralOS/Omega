// ============================================================================
// Alpha tape VM — macOS arm64 (Mach-O).  Hand-authored; the per-platform seed.
//
// A faithful, independently written realization of the same 21-opcode semantics
// as the x64 VM.  Running identical tapes on a different ISA and OS supplies
// useful cross-platform conformance evidence.  Agreement is not DDC and does
// not grant either realization authority; the written semantics and each
// realization's audited correspondence to them are the trust boundary.
//
// Trust obligation (source/alpha/README.md): disassemble the committed binary
// and read it against THIS source.  alpha_arm64_macos.lst is a committed
// disassembly to ease that audit.
//
// VM model (observable semantics identical to the x64 seed):
//   vregs[]  64-bit register file, byte-indexed            (bss, x19)
//   mem[]    flat ~256 MB zeroed byte memory; tape at [0]   (bss, x20)
//   pc       absolute pointer into mem                      (x21)
//   sp       call-stack byte offset, grows down from 256 MB (x22)
// A program tape [4-byte LE length][bytecode] is stamped into the __tape hole;
// the loader copies it to mem[0], pc=0, dispatches.  Byte I/O via read(0)/
// write(1) (libSystem — the analog of the x64 seed's kernel32 ReadFile/
// WriteFile imports); halt -> exit(low32 of vregs[rD]); unknown opcode -> trap.
//
// Opcodes: 00 halt 01 imm 02 mov 03 add 04 sub 05 mul 06 div 07 mod 08 loadb
//   09 storeb 0A load 0B store 0C jmp 0D jz 0E jnz 0F jlt(signed) 10 jeq
//   11 read 12 write 13 call 14 ret.
//
// macOS wrinkle: dd-stamping a tape invalidates the Mach-O code signature, so a
// stamped seed must be re-signed (codesign -f -s -) before it will exec.
//
// Build (reproducible — -no_uuid drops the nondeterministic Mach-O UUID, so the
// binary is byte-identical across builds modulo the OS code signature):
//   clang -arch arm64 -Wl,-no_uuid -o alpha_arm64_macos alpha_arm64_macos.s
// `verify.sh` re-derives the committed binary this way and checks it matches.
// ============================================================================
.global _main
.align 4
_main:
    stp x29, x30, [sp, #-16]!
    stp x19, x20, [sp, #-16]!
    stp x21, x22, [sp, #-16]!
    stp x23, x24, [sp, #-16]!
    adrp x19, vregs@PAGE
    add  x19, x19, vregs@PAGEOFF
    adrp x20, mem@PAGE
    add  x20, x20, mem@PAGEOFF
    adrp x9, _tape@PAGE
    add  x9, x9, _tape@PAGEOFF
    ldr  w10, [x9]
    add  x9, x9, #4
    mov  x11, #0
Lcopy:
    cbz  w10, Lcopied
    ldrb w12, [x9], #1
    strb w12, [x20, x11]
    add  x11, x11, #1
    sub  w10, w10, #1
    b    Lcopy
Lcopied:
    mov  x21, x20
    movz x22, #0x1000, lsl #16
    b    next
h_imm:
    // Decode the adjacent destination byte and unaligned immediate without a
    // program-counter writeback dependency, then advance by the exact 9 bytes.
    // As the hottest handler, it falls through directly into dispatch.
    ldrb w9,  [x21]
    ldr  x10, [x21, #1]
    add  x21, x21, #9
    str  x10, [x19, w9, uxtw #3]
next:
    ldrb w23, [x21], #1
    cmp  w23, #1
    b.eq h_imm
    cmp  w23, #4
    b.eq h_sub
    cmp  w23, #10
    b.eq h_load
    cmp  w23, #2
    b.eq h_mov
    cmp  w23, #11
    b.eq h_store
    cmp  w23, #3
    b.eq h_add
    cmp  w23, #12
    b.eq h_jmp
    cmp  w23, #13
    b.eq h_jz
    cmp  w23, #16
    b.eq h_jeq
    cmp  w23, #19
    b.eq h_call
    cmp  w23, #20
    b.eq h_ret
    cmp  w23, #15
    b.eq h_jlt
    cmp  w23, #5
    b.eq h_mul
    cmp  w23, #8
    b.eq h_loadb
    cmp  w23, #6
    b.eq h_div
    cmp  w23, #9
    b.eq h_storeb
    cmp  w23, #7
    b.eq h_mod
    cmp  w23, #17
    b.eq h_read
    cmp  w23, #0
    b.eq h_halt
    cmp  w23, #14
    b.eq h_jnz
    cmp  w23, #18
    b.eq h_write
    udf  #0
// Hot two-register handlers read the adjacent operand bytes independently,
// then advance pc once.  This is the same d,s decode and pc+2 transition as
// two serial post-index loads, without a load-to-load writeback dependency.
h_mov:
    ldrb w9,  [x21]
    ldrb w10, [x21, #1]
    add  x21, x21, #2
    ldr  x11, [x19, w10, uxtw #3]
    str  x11, [x19, w9,  uxtw #3]
    b    next
h_add:
    ldrb w9,  [x21]
    ldrb w10, [x21, #1]
    add  x21, x21, #2
    ldr  x11, [x19, w9,  uxtw #3]
    ldr  x12, [x19, w10, uxtw #3]
    add  x11, x11, x12
    str  x11, [x19, w9,  uxtw #3]
    b    next
h_sub:
    ldrb w9,  [x21]
    ldrb w10, [x21, #1]
    add  x21, x21, #2
    ldr  x11, [x19, w9,  uxtw #3]
    ldr  x12, [x19, w10, uxtw #3]
    sub  x11, x11, x12
    str  x11, [x19, w9,  uxtw #3]
    b    next
h_mul:
    ldrb w9,  [x21], #1
    ldrb w10, [x21], #1
    ldr  x11, [x19, w9,  uxtw #3]
    ldr  x12, [x19, w10, uxtw #3]
    mul  x11, x11, x12
    str  x11, [x19, w9,  uxtw #3]
    b    next
h_div:                            // signed; div-by-zero and INT_MIN/-1 -> trap
    ldrb w9,  [x21], #1
    ldrb w10, [x21], #1
    ldr  x11, [x19, w9,  uxtw #3]
    ldr  x12, [x19, w10, uxtw #3]
    cbz  x12, Ldz                  // /0 -> trap (matches x86 idiv #DE)
    cmn  x12, #1                   // divisor == -1 ?
    b.ne Ldiv_ok
    movz x13, #0x8000, lsl #48     // INT64_MIN
    cmp  x11, x13
    b.eq Ldz                       // INT_MIN / -1 overflow -> trap (x86 idiv #DE)
Ldiv_ok:
    sdiv x11, x11, x12
    str  x11, [x19, w9,  uxtw #3]
    b    next
Ldz:
    udf  #0
h_mod:                            // signed remainder; same two trap conditions
    ldrb w9,  [x21], #1
    ldrb w10, [x21], #1
    ldr  x11, [x19, w9,  uxtw #3]
    ldr  x12, [x19, w10, uxtw #3]
    cbz  x12, Ldz
    cmn  x12, #1
    b.ne Lmod_ok
    movz x14, #0x8000, lsl #48
    cmp  x11, x14
    b.eq Ldz
Lmod_ok:
    sdiv x13, x11, x12
    msub x11, x13, x12, x11
    str  x11, [x19, w9,  uxtw #3]
    b    next
h_loadb:
    ldrb w9,  [x21], #1
    ldrb w10, [x21], #1
    ldr  x12, [x19, w10, uxtw #3]
    ldrb w11, [x20, x12]
    str  x11, [x19, w9,  uxtw #3]
    b    next
h_storeb:
    ldrb w9,  [x21], #1
    ldrb w10, [x21], #1
    ldr  x11, [x19, w9,  uxtw #3]
    ldr  x12, [x19, w10, uxtw #3]
    strb w12, [x20, x11]
    b    next
h_load:
    ldrb w9,  [x21]
    ldrb w10, [x21, #1]
    add  x21, x21, #2
    ldr  x12, [x19, w10, uxtw #3]
    ldr  x11, [x20, x12]
    str  x11, [x19, w9,  uxtw #3]
    b    next
h_store:
    ldrb w9,  [x21]
    ldrb w10, [x21, #1]
    add  x21, x21, #2
    ldr  x11, [x19, w9,  uxtw #3]
    ldr  x12, [x19, w10, uxtw #3]
    str  x12, [x20, x11]
    b    next
h_jmp:
    ldr  x10, [x21]
    add  x21, x20, x10
    b    next
h_jz:
    ldrb w9,  [x21], #1
    ldr  x10, [x21]
    ldr  x11, [x19, w9, uxtw #3]
    cbnz x11, Ljz_skip
    add  x21, x20, x10
    b    next
Ljz_skip:
    add  x21, x21, #8
    b    next
h_jnz:
    ldrb w9,  [x21], #1
    ldr  x10, [x21]
    ldr  x11, [x19, w9, uxtw #3]
    cbz  x11, Ljnz_skip
    add  x21, x20, x10
    b    next
Ljnz_skip:
    add  x21, x21, #8
    b    next
h_jlt:
    ldrb w9,  [x21], #1
    ldrb w10, [x21], #1
    ldr  x12, [x21]
    ldr  x13, [x19, w9,  uxtw #3]
    ldr  x14, [x19, w10, uxtw #3]
    cmp  x13, x14
    b.lt Ljlt_take
    add  x21, x21, #8
    b    next
Ljlt_take:
    add  x21, x20, x12
    b    next
h_jeq:
    ldrb w9,  [x21], #1
    ldrb w10, [x21], #1
    ldr  x12, [x21]
    ldr  x13, [x19, w9,  uxtw #3]
    ldr  x14, [x19, w10, uxtw #3]
    cmp  x13, x14
    b.eq Ljeq_take
    add  x21, x21, #8
    b    next
Ljeq_take:
    add  x21, x20, x12
    b    next
h_read:
    ldrb w24, [x21], #1
    mov  x0, #0
    adrp x1, io_byte@PAGE
    add  x1, x1, io_byte@PAGEOFF
    mov  x2, #1
    bl   _read
    cmp  x0, #1
    b.lt Lrd_eof
    adrp x1, io_byte@PAGE
    add  x1, x1, io_byte@PAGEOFF
    ldrb w9, [x1]
    str  x9, [x19, w24, uxtw #3]
    b    next
Lrd_eof:
    mov  x9, #-1
    str  x9, [x19, w24, uxtw #3]
    b    next
h_write:
    ldrb w9,  [x21], #1
    ldr  x10, [x19, w9, uxtw #3]
    adrp x1, io_byte@PAGE
    add  x1, x1, io_byte@PAGEOFF
    strb w10, [x1]
    mov  x0, #1
    mov  x2, #1
    bl   _write
    b    next
h_call:
    ldr  x10, [x21]
    add  x11, x21, #8
    sub  x11, x11, x20
    sub  x22, x22, #8
    str  x11, [x20, x22]
    add  x21, x20, x10
    b    next
h_ret:
    ldr  x11, [x20, x22]
    add  x22, x22, #8
    add  x21, x20, x11
    b    next
h_halt:
    ldrb w9, [x21], #1
    ldr  x10, [x19, w9, uxtw #3]
    mov  w0, w10
    ldp  x23, x24, [sp], #16
    ldp  x21, x22, [sp], #16
    ldp  x19, x20, [sp], #16
    ldp  x29, x30, [sp], #16
    ret
.zerofill __DATA,__bss,vregs,0x800,3
.zerofill __DATA,__bss,mem,0x10010000,4
.zerofill __DATA,__bss,io_byte,8,3
.section __DATA,__tape
.global _tape
_tape:
.long 0
.space 0xfffffc
