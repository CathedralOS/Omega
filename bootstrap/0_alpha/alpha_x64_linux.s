# =============================================================================
# Alpha tape VM — Linux x86-64 ELF64 seed (AlphaBootstrapV5).
#
# This is the Linux realization of the same audited interpreter carried by
# alpha_x64_windows.hex: identical register convention, opcode dispatch,
# bounds checks, and handler code; only the host service calls differ
# (VirtualAlloc/GetStdHandle/ReadFile/WriteFile/ExitProcess become the mmap,
# read, write, and exit syscalls). Audit correspondence is against SEMANTICS.md
# and the sibling seed's handler bodies; the committed .lst disassembly pins
# the emitted bytes.
#
# The VM carries the program INSIDE itself: the .tape section is a 16 MiB hole
# of zeros. On startup the VM copies the embedded tape ([4-byte LE length][raw
# tape]) into VM memory and runs it; stdin/stdout are then purely the program's
# own I/O. Stamping copies this file and memcpy's the program into the hole.
#
#   rsi = pc ptr      rdi = M base (mmap result)   rbx = &vregs
#   r12 = sp          r13 = MEMSIZE (0x2000000000)  r14 = MEMSIZE-8
#   vregs occupy .data start .. +0x7ff; io_byte scratch at +0x800.
#   syscall clobbers only rcx/r11, both dead across the I/O calls.
#
# Layout (ld defaults, --build-id=none): headers+text at 0x400000 (R-X),
# .data at 0x402000 (RW, vregs+scratch), .tape at 0x403000 (RW hole).
# =============================================================================

    .intel_syntax noprefix

    .section .text
    .globl _start
_start:
    sub rsp, 0x28                        # shadow/diagnostic parity with the PE entry
    mov rbx, OFFSET vregs                # &vregs
    mov r12, 0x10000000                  # sp = 256 MiB stack origin
# --- obtain M: mmap(NULL, MEMSIZE, PROT_READ|PROT_WRITE,
#                    MAP_PRIVATE|MAP_ANONYMOUS|MAP_NORESERVE, -1, 0) supplies
#     the flat zeroed extent. NORESERVE matches the PE's MEM_RESERVE+lazy
#     commit: the reservation must not be swap-accounted at startup.
#     A refusal (negative -errno) is a startup trap ---
    xor edi, edi                         # addr = NULL
    movabs rsi, 0x2000000000             # len = MEMSIZE
    mov edx, 3                           # PROT_READ|PROT_WRITE
    mov r10d, 0x4022                     # MAP_PRIVATE|ANONYMOUS|NORESERVE
    mov r8, -1                           # fd
    xor r9d, r9d                         # offset
    mov eax, 9                           # SYS_mmap
    syscall
    test rax, rax
    js trap                              # -errno: extent unobtainable -> trap
    mov rdi, rax                         # mem base
# --- loader: copy the embedded tape (the .tape hole) into VM memory ---
    mov rax, OFFSET tape_hole
    mov r14d, dword ptr [rax]            # tape length, 4-byte LE
    cmp r14d, 0xfffffc                   # hole limit < MEMSIZE
    ja trap                              # reject before copying any tape byte
    add rax, 4                           # rax -> first tape byte
    xor r15d, r15d                       # i = 0
load_loop:
    test r14, r14
    jz load_done
    movzx ecx, byte ptr [rax]
    mov rdx, rdi
    add rdx, r15
    mov byte ptr [rdx], cl
    inc rax
    inc r15
    dec r14
    jmp load_loop
load_done:
    mov rsi, rdi                         # pc = 0
    movabs r13, 0x2000000000             # MEMSIZE
    movabs r14, 0x1ffffffff8             # MEMSIZE-8

# --- fetch + dispatch ---
next:
    mov rdx, rsi
    sub rdx, rdi                         # recover the unsigned pc offset
    cmp rdx, r13                         # MEMSIZE; also rejects wrapped targets
    jae trap                             # opcode must be in memory
    movzx eax, byte ptr [rsi]
    cmp eax, 0x14
    ja trap                              # unknown opcode; no width-table access
    lea r8, [rip+widths]
    movzx ecx, byte ptr [r8+rax]
    add rdx, rcx                         # offset + full width cannot wrap
    cmp rdx, r13                         # MEMSIZE
    ja trap                              # check every operand before any effect
    inc rsi
    cmp eax, 0x00
    je h_halt
    cmp eax, 0x01
    je h_imm
    cmp eax, 0x02
    je h_mov
    cmp eax, 0x03
    je h_add
    cmp eax, 0x04
    je h_sub
    cmp eax, 0x05
    je h_mul
    cmp eax, 0x06
    je h_div
    cmp eax, 0x07
    je h_mod
    cmp eax, 0x08
    je h_loadb
    cmp eax, 0x09
    je h_storeb
    cmp eax, 0x0a
    je h_load
    cmp eax, 0x0b
    je h_store
    cmp eax, 0x0c
    je h_jmp
    cmp eax, 0x0d
    je h_jz
    cmp eax, 0x0e
    je h_jnz
    cmp eax, 0x0f
    je h_jlt
    cmp eax, 0x10
    je h_jeq
    cmp eax, 0x11
    je h_read
    cmp eax, 0x12
    je h_write
    cmp eax, 0x13
    je h_call
    cmp eax, 0x14
    je h_ret
trap:
    ud2                                  # abnormal halt; preserves prior stdout

do_jump:
    mov rax, qword ptr [rsi]
    mov rsi, rdi
    add rsi, rax
    jmp next
skip_addr:
    add rsi, 8
    jmp next

h_imm:
    movzx ecx, byte ptr [rsi]
    inc rsi
    mov rax, qword ptr [rsi]
    add rsi, 8
    mov qword ptr [rbx+rcx*8], rax
    jmp next
h_mov:
    movzx ecx, byte ptr [rsi]
    inc rsi
    movzx edx, byte ptr [rsi]
    inc rsi
    mov rax, qword ptr [rbx+rdx*8]
    mov qword ptr [rbx+rcx*8], rax
    jmp next
h_add:
    movzx ecx, byte ptr [rsi]
    inc rsi
    movzx edx, byte ptr [rsi]
    inc rsi
    mov rax, qword ptr [rbx+rcx*8]
    add rax, qword ptr [rbx+rdx*8]
    mov qword ptr [rbx+rcx*8], rax
    jmp next
h_sub:
    movzx ecx, byte ptr [rsi]
    inc rsi
    movzx edx, byte ptr [rsi]
    inc rsi
    mov rax, qword ptr [rbx+rcx*8]
    sub rax, qword ptr [rbx+rdx*8]
    mov qword ptr [rbx+rcx*8], rax
    jmp next
h_mul:
    movzx ecx, byte ptr [rsi]
    inc rsi
    movzx edx, byte ptr [rsi]
    inc rsi
    mov rax, qword ptr [rbx+rcx*8]
    imul rax, qword ptr [rbx+rdx*8]
    mov qword ptr [rbx+rcx*8], rax
    jmp next
h_div:
    movzx ecx, byte ptr [rsi]
    inc rsi
    movzx edx, byte ptr [rsi]
    inc rsi
    mov r8, qword ptr [rbx+rdx*8]
    mov rax, qword ptr [rbx+rcx*8]
    test r8, r8
    jz trap                             # /0 -> Trap (arm64 sibling's Ldz;
                                        # the PE leaks #DE instead)
    cmp r8, -1
    jne div_ok
    movabs r9, 0x8000000000000000
    cmp rax, r9
    je trap                             # INT64_MIN / -1 -> Trap
div_ok:
    cqo
    idiv r8
    mov qword ptr [rbx+rcx*8], rax
    jmp next
h_mod:
    movzx ecx, byte ptr [rsi]
    inc rsi
    movzx edx, byte ptr [rsi]
    inc rsi
    mov r8, qword ptr [rbx+rdx*8]
    mov rax, qword ptr [rbx+rcx*8]
    test r8, r8
    jz trap                             # /0 -> Trap
    cmp r8, -1
    jne mod_ok
    movabs r9, 0x8000000000000000
    cmp rax, r9
    je trap                             # INT64_MIN % -1 -> Trap
mod_ok:
    cqo
    idiv r8
    mov qword ptr [rbx+rcx*8], rdx
    jmp next
h_loadb:
    movzx ecx, byte ptr [rsi]
    inc rsi
    movzx edx, byte ptr [rsi]
    inc rsi
    mov rax, qword ptr [rbx+rdx*8]
    cmp rax, r13                         # MEMSIZE
    jae trap                             # unsigned byte address
    movzx edx, byte ptr [rdi+rax]
    mov qword ptr [rbx+rcx*8], rdx
    jmp next
h_storeb:
    movzx ecx, byte ptr [rsi]
    inc rsi
    movzx edx, byte ptr [rsi]
    inc rsi
    mov rax, qword ptr [rbx+rcx*8]
    mov rdx, qword ptr [rbx+rdx*8]
    cmp rax, r13                         # MEMSIZE
    jae trap
    mov byte ptr [rdi+rax], dl
    jmp next
h_load:
    movzx ecx, byte ptr [rsi]
    inc rsi
    movzx edx, byte ptr [rsi]
    inc rsi
    mov rax, qword ptr [rbx+rdx*8]
    cmp rax, r14                         # MEMSIZE-8
    ja trap                              # full word, without address addition
    mov rdx, qword ptr [rdi+rax]
    mov qword ptr [rbx+rcx*8], rdx
    jmp next
h_store:
    movzx ecx, byte ptr [rsi]
    inc rsi
    movzx edx, byte ptr [rsi]
    inc rsi
    mov rax, qword ptr [rbx+rcx*8]
    mov rdx, qword ptr [rbx+rdx*8]
    cmp rax, r14                         # MEMSIZE-8
    ja trap
    mov qword ptr [rdi+rax], rdx
    jmp next
h_jmp:
    jmp do_jump
h_jz:
    movzx ecx, byte ptr [rsi]
    inc rsi
    mov rax, qword ptr [rbx+rcx*8]
    test rax, rax
    jz do_jump
    jmp skip_addr
h_jnz:
    movzx ecx, byte ptr [rsi]
    inc rsi
    mov rax, qword ptr [rbx+rcx*8]
    test rax, rax
    jnz do_jump
    jmp skip_addr
h_jlt:
    movzx ecx, byte ptr [rsi]
    inc rsi
    movzx edx, byte ptr [rsi]
    inc rsi
    mov rax, qword ptr [rbx+rcx*8]
    cmp rax, qword ptr [rbx+rdx*8]
    jl do_jump
    jmp skip_addr
h_jeq:
    movzx ecx, byte ptr [rsi]
    inc rsi
    movzx edx, byte ptr [rsi]
    inc rsi
    mov rax, qword ptr [rbx+rcx*8]
    cmp rax, qword ptr [rbx+rdx*8]
    je do_jump
    jmp skip_addr
h_read:
    inc rsi                              # consume rD; rD now at rsi-1
    call getbyte                         # rax = byte or -1
    movzx ecx, byte ptr [rsi-1]
    mov qword ptr [rbx+rcx*8], rax
    jmp next
h_write:
    movzx ecx, byte ptr [rsi]            # rS
    inc rsi
    mov rax, qword ptr [rbx+rcx*8]       # al = byte
    call putbyte
    jmp next
h_call:
    lea rdx, [r12-8]                     # prospective stack offset
    cmp rdx, r14                         # MEMSIZE-8; underflow is unsigned large
    ja trap                              # no sp change or write on failed range
    mov rax, qword ptr [rsi]
    mov rdx, rsi
    sub rdx, rdi
    add rdx, 8
    sub r12, 8
    mov r10, rdi
    add r10, r12
    mov qword ptr [r10], rdx
    mov rsi, rdi
    add rsi, rax
    jmp next
h_ret:
    cmp r12, r14                         # MEMSIZE-8
    ja trap                              # no preceding-call or stack-origin restriction
    mov r10, rdi
    add r10, r12
    mov rdx, qword ptr [r10]
    add r12, 8
    mov rsi, rdi
    add rsi, rdx
    jmp next
h_halt:
    movzx ecx, byte ptr [rsi]            # rD operand
    mov edi, dword ptr [rbx+rcx*8]       # exit code = low bits of vregs[rD]
    mov eax, 60                          # SYS_exit
    syscall

# --- getbyte: rax = next stdin byte, or -1 at EOF / read failure ---
# read(0, &io_byte, 1): exactly one byte read keeps rax=1; EOF (0) or any
# failure (-errno) lands on the same EOF observation as the PE's ReadFile path,
# whose unchecked call sees count=0 on failure.
getbyte:
    push rsi                             # save VM pc ptr / mem base (syscall args)
    push rdi
    xor edi, edi                         # fd = stdin
    mov esi, OFFSET io_byte
    mov edx, 1
    xor eax, eax                         # SYS_read
    syscall
    pop rdi
    pop rsi
    cmp rax, 1
    jne gb_eof
    movzx eax, byte ptr [rip+io_byte]
    ret
gb_eof:
    mov rax, -1
    ret

# --- putbyte: write al to stdout (single-byte write; result ignored, same as
#     the PE's unchecked WriteFile) ---
putbyte:
    mov byte ptr [rip+io_byte], al
    push rsi
    push rdi
    mov edi, 1                           # fd = stdout
    mov esi, OFFSET io_byte
    mov edx, 1
    mov eax, 1                           # SYS_write
    syscall
    pop rdi
    pop rsi
    ret

# Total instruction widths, indexed only after op <= 0x14.
widths:
    .byte 2, 10, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 9, 10, 10, 11, 11, 2, 2, 9, 1

# =============================================================================
# .data — 256 vector registers + I/O scratch page (RW)
# =============================================================================
    .section .data
    .p2align 12
vregs:
    .fill 0x800, 1, 0                    # R[0..255], initially all zero
io_byte:
    .fill 8, 1, 0                        # one-byte read/write buffer
    .p2align 12                          # keep the tape hole page-aligned

# =============================================================================
# .tape — the embedded-program hole: 16 MiB including the four-byte length.
# A built program is this file with [4-byte LE length][tape bytes] memcpy'd in
# at the hole's file offset.
# =============================================================================
    .section .tape,"aw"
    .globl tape_hole
tape_hole:
    .fill 0x1000000, 1, 0

    .section .note.GNU-stack,"",@progbits
