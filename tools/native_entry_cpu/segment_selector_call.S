/* The harness retains its GDT through all eight ordinary compiled calls. */
#include "boot.inc"
 movb $0xff, %al
 outb %al, $0x21
 outb %al, $0xa1
 cli
 cld
 leaq buffers(%rip), %rdi
 movq $128, %rcx
 movb $0xa5, %al
 rep stosb
 movabsq $0x1122334455667788, %rax
 movq %rax, buffers(%rip)
 movq %rax, buffers+64(%rip)
 movq %rax, buffers+48(%rip)
 movq %rax, buffers+56(%rip)
 movq %rax, boot_guard_low(%rip)
 movq %rax, boot_guard_high(%rip)
 movq $0, buffers+8(%rip)
 movabsq $0x00af9a000000ffff, %rax
 movq %rax, buffers+16(%rip)
 movq %rax, buffers+32(%rip)
 movabsq $0x00cf92000000ffff, %rax
 movq %rax, buffers+24(%rip)
 movq %rax, buffers+40(%rip)
 leaq buffers+8(%rip), %rsi
 leaq buffers+72(%rip), %rdi
 movq $7, %rcx
 rep movsq
 movw $55, installed_gdtr(%rip)
 leaq buffers+8(%rip), %rax
 movq %rax, installed_gdtr+2(%rip)
 lgdt installed_gdtr(%rip)
 /* Distinct selectors and nonzero bases make no-op or implicit-null behavior
    fail. Bases are canonical but never dereferenced; KernelGSBase stays untouched. */
 movw $16, %ax
 movw %ax, %fs
 movw %ax, %gs
 movl $0xc0000100, %ecx
 movl $0x20000000, %eax
 movl $1, %edx
 wrmsr
 movl $0xc0000101, %ecx
 movl $0x21000000, %eax
 movl $0xffff8000, %edx
 wrmsr
 movl $0xc0000102, %ecx
 movl $0x22000000, %eax
 movl $2, %edx
 wrmsr
 movq $0, round(%rip)
 movb $'B', %al
 outb %al, $0xe9
next_round:
 movq %rsp, expected_rsp(%rip)
 movq $0x2222, %rbx
 movq $0x7777, %rbp
 movq $0xcccc, %r12
 movq $0xdddd, %r13
 movq $0xeeee, %r14
 movq $0xffff, %r15
 leaq buffers(%rip), %rdi
 call payload_entry
 pushfq
 popq observed_flags(%rip)
 movq %rax, observed_result(%rip)
 cli
 cld
 cmpq expected_rsp(%rip), %rsp
 jne fail_stack
 cmpq $0x2222, %rbx
 jne fail_gpr
 cmpq $0x7777, %rbp
 jne fail_gpr
 cmpq $0xcccc, %r12
 jne fail_gpr
 cmpq $0xdddd, %r13
 jne fail_gpr
 cmpq $0xeeee, %r14
 jne fail_gpr
 cmpq $0xffff, %r15
 jne fail_gpr
 testq $0x600, observed_flags(%rip)
 jne fail_flags
 cmpq $0, observed_result(%rip)
 jne fail_result
 movw %cs, %ax
 cmpw $24, %ax
 jne fail_selector
 movw %ds, %ax
 cmpw $32, %ax
 jne fail_selector
 movw %es, %ax
 cmpw $32, %ax
 jne fail_selector
 movw %ss, %ax
 cmpw $32, %ax
 jne fail_selector
 movw %fs, %ax
 testw %ax, %ax
 jne fail_selector
 movw %gs, %ax
 testw %ax, %ax
 jne fail_selector
 movl $0xc0000100, %ecx
 rdmsr
 cmpl $0x20000000, %eax
 jne fail_base
 cmpl $1, %edx
 jne fail_base
 incl %ecx
 rdmsr
 cmpl $0x21000000, %eax
 jne fail_base
 cmpl $0xffff8000, %edx
 jne fail_base
 incl %ecx
 rdmsr
 cmpl $0x22000000, %eax
 jne fail_base
 cmpl $2, %edx
 jne fail_base
 sgdt observed_gdtr(%rip)
 leaq installed_gdtr(%rip), %rsi
 leaq observed_gdtr(%rip), %rdi
 movq $10, %rcx
 repe cmpsb
 jne fail_selector
 cmpq $0, buffers+8(%rip)
 jne fail_neighbor
 movabsq $0x00af9a000000ffff, %rax
 cmpq %rax, buffers+16(%rip)
 jne fail_neighbor
 movabsq $0x00cf93000000ffff, %rax
 cmpq %rax, buffers+24(%rip)
 jne fail_neighbor
 /* QEMU 11.0.2 RET does not write CS's accessed bit. Observe CS separately
    and require every descriptor bit except that hardware-owned bit unchanged. */
 movq buffers+32(%rip), %rdx
 btrq $40, %rdx
 movabsq $0x00af9a000000ffff, %rax
 cmpq %rax, %rdx
 jne fail_neighbor
 movabsq $0x00cf93000000ffff, %rax
 cmpq %rax, buffers+40(%rip)
 jne fail_neighbor
#ifdef CORRUPT_NEIGHBOR
 movb $0, buffers+64(%rip)
#endif
 movabsq $0x1122334455667788, %rax
 cmpq %rax, buffers(%rip)
 jne fail_neighbor
 cmpq %rax, buffers+48(%rip)
 jne fail_neighbor
 cmpq %rax, buffers+56(%rip)
 jne fail_neighbor
 cmpq %rax, buffers+64(%rip)
 jne fail_neighbor
 cmpq %rax, buffers+112(%rip)
 jne fail_neighbor
 cmpq %rax, buffers+120(%rip)
 jne fail_neighbor
 cmpq %rax, boot_guard_low(%rip)
 jne fail_stack
 cmpq %rax, boot_guard_high(%rip)
 jne fail_stack
 cmpq $0, buffers+72(%rip)
 jne fail_neighbor
 movabsq $0x00af9a000000ffff, %rax
 cmpq %rax, buffers+80(%rip)
 jne fail_neighbor
 cmpq %rax, buffers+96(%rip)
 jne fail_neighbor
 movabsq $0x00cf92000000ffff, %rax
 cmpq %rax, buffers+88(%rip)
 jne fail_neighbor
 cmpq %rax, buffers+104(%rip)
 jne fail_neighbor
 incq round(%rip)
 movb $'.', %al
 outb %al, $0xe9
 cmpq $8, round(%rip)
 jne next_round
 movb $'P', %al
 outb %al, $0xe9
 movl $0x10, %eax
 jmp exit
fail_gpr: movb $'G', %al; jmp fail
fail_flags: movb $'F', %al; jmp fail
fail_stack: movb $'S', %al; jmp fail
fail_result: movb $'R', %al; jmp fail
fail_selector: movb $'C', %al; jmp fail
fail_base: movb $'B', %al; jmp fail
fail_neighbor: movb $'N', %al
fail:
 outb %al, $0xe9
 movl $0x11, %eax
exit:
 movw $0xf4, %dx
 outl %eax, %dx
 cli
2: hlt; jmp 2b
.balign 8
gdt: .quad 0, 0x00af9a000000ffff, 0x00cf92000000ffff
gdt_pointer: .word 23; .long gdt
.section .bss,"aw",@nobits
.balign 4096
pml4: .skip 4096
pdpt: .skip 4096
page_directory: .skip 4096
.balign 16
buffers: .skip 128
installed_gdtr: .skip 10
observed_gdtr: .skip 10
.balign 8
observed_flags: .skip 8
observed_result: .skip 8
expected_rsp: .skip 8
round: .skip 8
boot_guard_low: .skip 8
.balign 16
boot_stack: .skip 32768
boot_stack_top:
boot_guard_high: .skip 8
.section .payload,"ax"
.global payload
payload:
.incbin "payload.bin"
.global payload_entry
.set payload_entry, payload + ENTRY_OFFSET
