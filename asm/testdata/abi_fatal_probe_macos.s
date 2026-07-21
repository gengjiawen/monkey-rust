// Handwritten fatal-path probe, Mach-O dialect (design §10.3): the macOS
// twin of abi_fatal_probe.s. rt_add on i64::MAX + 1 must terminate through
// rt_fatal semantics — "monkey: IntegerOverflow: ..." on stderr and exit
// code 1. The trailing exit-0 epilogue must be unreachable.

    .text
    .globl _main
    .p2align 2
_main:
    stp x29, x30, [sp, #-16]!
    mov x29, sp

    adrp x0, g_globals@PAGE
    add x0, x0, g_globals@PAGEOFF
    movz x1, #0
    bl _rt_globals_init

    movz x0, #0xffff                 // i64::MAX = 0x7fffffffffffffff
    movk x0, #0xffff, lsl #16
    movk x0, #0xffff, lsl #32
    movk x0, #0x7fff, lsl #48
    bl _rt_box_int
    movz x1, #2                      // SMI 1
    bl _rt_add                       // fatal: IntegerOverflow

    mov w0, #0                       // unreachable
    mov sp, x29
    ldp x29, x30, [sp], #16
    ret

    .zerofill __DATA,__bss,g_globals,8,3
