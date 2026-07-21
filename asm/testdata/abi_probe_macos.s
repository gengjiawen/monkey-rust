// Handwritten ABI probe, Mach-O dialect (design §10.3): the macOS twin of
// abi_probe.s — identical instruction stream, Darwin spelling (`_`-prefixed
// C symbols, `L` local labels, @PAGE/@PAGEOFF, __TEXT,__const, .zerofill).
// Covers rt_globals_init, strings, builtin call via rt_call, closures with
// 0 / 2 / 7 parameters (the full x1..x7 argument range), SMI ↔ boxed
// integer equality, and the rt_construct/rt_call distinction.
//
// Expected stdout: "abi\n3\n28\n7\n", exit code 0 (42 on assertion failure).

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

    // puts("abi") through rt_call: builtin puts = (1 << 3) | 0b101 = 0xd.
    adrp x0, Lstr_abi@PAGE
    add x0, x0, Lstr_abi@PAGEOFF
    movz x1, #3
    bl _rt_string_from_bytes
    sub sp, sp, #16
    str x0, [sp, #8]                 // arg 0
    movz x0, #0xd
    str x0, [sp]                     // callee
    ldr x0, [sp]
    movz x1, #1
    add x2, sp, #8
    bl _rt_call
    add sp, sp, #16

    // add2 = closure(Lfn_add2, 2 params); puts(add2(1, 2)) -> "3".
    adrp x0, Lfn_add2@PAGE
    add x0, x0, Lfn_add2@PAGEOFF
    movz x1, #2
    mov x2, sp                       // no captures: base unused at len 0
    movz x3, #0
    bl _rt_closure
    sub sp, sp, #32
    str x0, [sp]                     // callee
    movz x0, #2                      // SMI 1
    str x0, [sp, #8]
    movz x0, #4                      // SMI 2
    str x0, [sp, #16]
    ldr x0, [sp]
    movz x1, #2
    add x2, sp, #8
    bl _rt_call
    add sp, sp, #32
    bl print_acc

    // sum7 = closure(Lfn_sum7, 7 params); puts(sum7(1..7)) -> "28".
    adrp x0, Lfn_sum7@PAGE
    add x0, x0, Lfn_sum7@PAGEOFF
    movz x1, #7
    mov x2, sp
    movz x3, #0
    bl _rt_closure
    sub sp, sp, #64                  // callee + 7 args
    str x0, [sp]
    movz x0, #2
    str x0, [sp, #8]
    movz x0, #4
    str x0, [sp, #16]
    movz x0, #6
    str x0, [sp, #24]
    movz x0, #8
    str x0, [sp, #32]
    movz x0, #10
    str x0, [sp, #40]
    movz x0, #12
    str x0, [sp, #48]
    movz x0, #14
    str x0, [sp, #56]
    ldr x0, [sp]
    movz x1, #7
    add x2, sp, #8
    bl _rt_call
    add sp, sp, #64
    bl print_acc

    // zero-argument closure -> SMI 7; puts prints "7".
    adrp x0, Lfn_zero@PAGE
    add x0, x0, Lfn_zero@PAGEOFF
    movz x1, #0
    mov x2, sp
    movz x3, #0
    bl _rt_closure
    sub sp, sp, #16
    str x0, [sp]
    ldr x0, [sp]
    movz x1, #0
    add x2, sp, #8
    bl _rt_call
    add sp, sp, #16
    bl print_acc

    // SMI <-> boxed: rt_box_int(5) == SMI 5 must be true (0b0111).
    movz x0, #5
    bl _rt_box_int
    movz x1, #10
    bl _rt_eq
    cmp x0, #7
    b.ne Lfail

    // rt_construct is selected by node kind, not callee type: a class with
    // no constructor and zero arguments yields a (truthy) instance.
    adrp x0, Lstr_cls@PAGE
    add x0, x0, Lstr_cls@PAGEOFF
    movz x1, #1
    bl _rt_class
    sub sp, sp, #16
    str x0, [sp]
    ldr x0, [sp]
    movz x1, #0
    add x2, sp, #8
    bl _rt_construct
    add sp, sp, #16
    bl _rt_truthy
    cmp x0, #1
    b.ne Lfail

    mov w0, #0
    mov sp, x29
    ldp x29, x30, [sp], #16
    ret
Lfail:
    mov w0, #42
    mov sp, x29
    ldp x29, x30, [sp], #16
    ret

// print_acc: puts(x0), preserving nothing (helper, not part of the ABI).
print_acc:
    stp x29, x30, [sp, #-16]!
    mov x29, sp
    sub sp, sp, #16
    str x0, [sp, #8]
    movz x0, #0xd
    str x0, [sp]
    ldr x0, [sp]
    movz x1, #1
    add x2, sp, #8
    bl _rt_call
    mov sp, x29
    ldp x29, x30, [sp], #16
    ret

// fn add2(closure x0, a x1, b x2) = a + b.
Lfn_add2:
    stp x29, x30, [sp, #-16]!
    mov x29, sp
    mov x0, x1
    mov x1, x2
    bl _rt_add
    mov sp, x29
    ldp x29, x30, [sp], #16
    ret

// fn sum7(closure x0, a1..a7 in x1..x7): arguments are caller-saved, so
// spill all seven before the first rt_add.
Lfn_sum7:
    stp x29, x30, [sp, #-16]!
    mov x29, sp
    sub sp, sp, #64
    str x1, [sp]
    str x2, [sp, #8]
    str x3, [sp, #16]
    str x4, [sp, #24]
    str x5, [sp, #32]
    str x6, [sp, #40]
    str x7, [sp, #48]
    ldr x0, [sp]
    ldr x1, [sp, #8]
    bl _rt_add
    ldr x1, [sp, #16]
    bl _rt_add
    ldr x1, [sp, #24]
    bl _rt_add
    ldr x1, [sp, #32]
    bl _rt_add
    ldr x1, [sp, #40]
    bl _rt_add
    ldr x1, [sp, #48]
    bl _rt_add
    mov sp, x29
    ldp x29, x30, [sp], #16
    ret

// fn zero(closure x0) = 7. Leaf: x30 stays intact, plain ret.
Lfn_zero:
    movz x0, #14
    ret

    .section __TEXT,__const
Lstr_abi:
    .byte 0x61, 0x62, 0x69
Lstr_cls:
    .byte 0x43

    .zerofill __DATA,__bss,g_globals,8,3
