/*
 * alpha_vm.c — host-instrumentation Alpha VM for the certificate check gate's
 * --reference-vm diagnostic leg.
 *
 * Implements the 21-opcode small-step machine from bootstrap/0_alpha/
 * SEMANTICS.md at the AlphaBootstrapV5 extent: a flat, initially-zeroed
 * M[0..MEMSIZE) obtained from one reserve-and-demand-zero anonymous mapping,
 * sp starting at 0x10000000, byte-granular stdin/stdout I/O, and the same
 * observable contract as tests/alpha/reference/alpha_ref.py — trap exits 132
 * (matching SIGILL -> shell 128+4) after flushing buffered output, and halt
 * exits with R[d] & 0xFF.
 *
 * This is an observation instrument, never an admission route: the selected
 * chain's audited seeds remain the only trusted evaluators. The diagnostic
 * leg records reference-VM figures so the profile ledger can compare them
 * with the first native run.
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>

#ifndef MAP_NORESERVE
#define MAP_NORESERVE 0                    /* absent on non-Linux hosts */
#endif

#define MEMSIZE 0x2000000000ULL            /* AlphaBootstrapV5 (128 GiB sparse) */
#define TAPE_MAX 0x00FFFFFCULL             /* 16 MiB stamped hole minus u32 len */
#define SP0 0x10000000ULL
#define MASK UINT64_MAX
#define INT64_MIN_S INT64_MIN

/* opcode operand widths (bytes total per instruction) */
static const uint8_t WIDTHS[21] = {
    2, 10, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 9, 10, 10, 11, 11, 2, 2, 9, 1
};

static uint8_t *M;
static uint64_t R[256];
static uint64_t sp;
static uint64_t pc;

/* buffered program output, flushed on halt or trap */
static uint8_t *out;
static size_t out_len, out_cap;

static const uint8_t *inp;
static size_t inp_len, ipos;

_Noreturn static void flush_and_exit(int code) {
    if (out_len) {
        size_t off = 0;
        while (off < out_len) {
            ssize_t n = fwrite(out + off, 1, out_len - off, stdout);
            if (n <= 0) break;
            off += (size_t)n;
        }
        fflush(stdout);
    }
    exit(code);
}

static void trap(void) { flush_and_exit(132); }

static void check_range(uint64_t a, uint64_t w) {
    if (a > MEMSIZE - w) trap();
}

static uint64_t rd8(uint64_t a) {
    uint64_t v;
    memcpy(&v, M + a, 8);
    return v;
}

static void wr8(uint64_t a, uint64_t v) {
    memcpy(M + a, &v, 8);
}

static void emit(uint8_t b) {
    if (out_len == out_cap) {
        out_cap = out_cap ? out_cap * 2 : 65536;
        out = realloc(out, out_cap);
        if (!out) exit(70);
    }
    out[out_len++] = b;
}

static int64_t s64(uint64_t x) { return (int64_t)x; }

static int64_t trunc_div(int64_t a, int64_t b) {
    int64_t q = a / b;                 /* C99: truncates toward zero */
    return q;
}

int main(int argc, char **argv) {
    if (argc != 2) {
        fprintf(stderr, "usage: alpha_vm TAPE\n");
        return 2;
    }
    FILE *f = fopen(argv[1], "rb");
    if (!f) { perror("tape"); return 2; }
    fseek(f, 0, SEEK_END);
    long tape_len = ftell(f);
    fseek(f, 0, SEEK_SET);
    if (tape_len < 0 || (uint64_t)tape_len > TAPE_MAX || (uint64_t)tape_len > MEMSIZE)
        trap();

    M = mmap(NULL, MEMSIZE, PROT_READ | PROT_WRITE,
             MAP_PRIVATE | MAP_ANONYMOUS | MAP_NORESERVE, -1, 0);
    if (M == MAP_FAILED) { perror("mmap"); return 2; }

    if (tape_len && fread(M, 1, (size_t)tape_len, f) != (size_t)tape_len) {
        perror("tape read");
        return 2;
    }
    fclose(f);

    /* buffered stdin: the request extent is bounded, so one read pass */
    size_t cap = 1 << 22;
    uint8_t *buf = malloc(cap);
    size_t len = 0;
    if (!buf) return 2;
    for (;;) {
        if (len == cap) {
            cap *= 2;
            buf = realloc(buf, cap);
            if (!buf) return 2;
        }
        ssize_t n = fread(buf + len, 1, cap - len, stdin);
        if (n <= 0) break;
        len += (size_t)n;
    }
    inp = buf;
    inp_len = len;

    sp = SP0;
    pc = 0;

    for (;;) {
        check_range(pc, 1);
        uint8_t op = M[pc];
        if (op >= 21) trap();
        check_range(pc, WIDTHS[op]);
        switch (op) {
        case 0x00:                                   /* halt d */
            flush_and_exit((int)(R[M[pc + 1]] & 0xFF));
        case 0x01:                                   /* imm d, k */
            R[M[pc + 1]] = rd8(pc + 2); pc += 10; break;
        case 0x02:                                   /* mov d, s */
            R[M[pc + 1]] = R[M[pc + 2]]; pc += 3; break;
        case 0x03:                                   /* add */
            R[M[pc + 1]] += R[M[pc + 2]]; pc += 3; break;
        case 0x04:                                   /* sub */
            R[M[pc + 1]] -= R[M[pc + 2]]; pc += 3; break;
        case 0x05:                                   /* mul */
            R[M[pc + 1]] *= R[M[pc + 2]]; pc += 3; break;
        case 0x06:                                   /* div (signed, trunc) */
        case 0x07: {                                 /* mod */
            uint8_t d = M[pc + 1];
            int64_t a = s64(R[d]), b = s64(R[M[pc + 2]]);
            if (b == 0 || (a == INT64_MIN_S && b == -1)) trap();
            int64_t q = trunc_div(a, b);
            R[d] = (uint64_t)(op == 0x06 ? q : a - q * b);
            pc += 3; break;
        }
        case 0x08:                                   /* loadb d, s */
            check_range(R[M[pc + 2]], 1);
            R[M[pc + 1]] = M[R[M[pc + 2]]]; pc += 3; break;
        case 0x09:                                   /* storeb d, s */
            check_range(R[M[pc + 1]], 1);
            M[R[M[pc + 1]]] = (uint8_t)R[M[pc + 2]]; pc += 3; break;
        case 0x0A:                                   /* load d, s */
            R[M[pc + 1]] = rd8(R[M[pc + 2]]); pc += 3; break;
        case 0x0B:                                   /* store d, s */
            wr8(R[M[pc + 1]], R[M[pc + 2]]); pc += 3; break;
        case 0x0C:                                   /* jmp a */
            pc = rd8(pc + 1); break;
        case 0x0D:                                   /* jz c, a */
            pc = R[M[pc + 1]] == 0 ? rd8(pc + 2) : pc + 10; break;
        case 0x0E:                                   /* jnz c, a */
            pc = R[M[pc + 1]] != 0 ? rd8(pc + 2) : pc + 10; break;
        case 0x0F:                                   /* jlt a, b, a2 */
            pc = s64(R[M[pc + 1]]) < s64(R[M[pc + 2]]) ? rd8(pc + 3) : pc + 11;
            break;
        case 0x10:                                   /* jeq a, b, a2 */
            pc = R[M[pc + 1]] == R[M[pc + 2]] ? rd8(pc + 3) : pc + 11; break;
        case 0x11:                                   /* read d */
            R[M[pc + 1]] = ipos < inp_len ? inp[ipos++] : MASK;
            pc += 2; break;
        case 0x12:                                   /* write s */
            emit((uint8_t)R[M[pc + 1]]); pc += 2; break;
        case 0x13: {                                 /* call a */
            uint64_t target = rd8(pc + 1);
            wr8(sp - 8, pc + 9); sp -= 8; pc = target; break;
        }
        case 0x14:                                   /* ret */
            pc = rd8(sp); sp += 8; break;
        }
    }
}
