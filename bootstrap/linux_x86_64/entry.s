# Linux gives _start the argument count followed by the argument vector.
.text
.global _start
_start:
    mov (%rsp), %rdi
    lea 8(%rsp), %rsi
    and $-16, %rsp
    call construct
    mov %eax, %edi
    mov $60, %eax                       # exit
    syscall

# The driver uses Linux calls with at most four arguments, without libc.
.global kernel
kernel:
    mov %rdi, %rax
    mov %rsi, %rdi
    mov %rdx, %rsi
    mov %rcx, %rdx
    mov %r8, %r10
    syscall
    ret

# The seed's 16 MiB .tape section contains only zeroes. A sparse file
# restores the section between these two exact regions when stamping.
.section .rodata
.global alpha_prefix, alpha_suffix, beta_root, beta_end
alpha_prefix:
    .incbin "bootstrap/interpreted/0_alpha/alpha_x64_linux", 0, 0x3000
alpha_suffix:
    .incbin "bootstrap/interpreted/0_alpha/alpha_x64_linux", 0x1003000
beta_root:
    .incbin "bootstrap/interpreted/1_beta/beta_compiler_bytecode.tape"
beta_end:
.section .note.GNU-stack,"",@progbits
