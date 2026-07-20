// Handwritten fatal-path probe (design §10.3): rt_add on i64::MAX + 1 must
// terminate through rt_fatal semantics — "monkey: IntegerOverflow: ..." on
// stderr and exit code 1. The trailing exit-0 epilogue must be unreachable.

    .text
    .globl main
    .p2align 2
main:
    stp x29, x30, [sp, #-16]!
    mov x29, sp

    adrp x0, g_globals
    add x0, x0, :lo12:g_globals
    movz x1, #0
    bl rt_globals_init

    movz x0, #0xffff                 // i64::MAX = 0x7fffffffffffffff
    movk x0, #0xffff, lsl #16
    movk x0, #0xffff, lsl #32
    movk x0, #0x7fff, lsl #48
    bl rt_box_int
    movz x1, #2                      // SMI 1
    bl rt_add                        // fatal: IntegerOverflow

    mov w0, #0                       // unreachable
    mov sp, x29
    ldp x29, x30, [sp], #16
    ret

    .bss
    .balign 8
g_globals:
    .skip 0
