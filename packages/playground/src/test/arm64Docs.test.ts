import { describe, expect, it } from 'vitest'

import { ARM64_MNEMONICS, arm64TokenAt, arm64TokenDoc } from '../arm64Docs'

// The complete `bl` surface of asm/lower.rs + asm/emitter.rs; a new runtime
// entry point must get a hover entry (the rt_* fallback would paper over it).
const EMITTED_RUNTIME_CALLS = [
  'rt_globals_init',
  'rt_string_from_bytes',
  'rt_box_int',
  'rt_array',
  'rt_hash',
  'rt_closure',
  'rt_get_free',
  'rt_class',
  'rt_class_add_method',
  'rt_get_property',
  'rt_set_property',
  'rt_index',
  'rt_add',
  'rt_sub',
  'rt_mul',
  'rt_div',
  'rt_eq',
  'rt_neq',
  'rt_gt',
  'rt_minus',
  'rt_bang',
  'rt_truthy',
  'rt_call',
  'rt_construct',
  'rt_observer_init',
  'rt_observe_result',
]

const EMITTED_DIRECTIVES = [
  '.text',
  '.globl',
  '.p2align',
  '.section',
  '.rodata',
  '.byte',
  '.bss',
  '.balign',
  '.skip',
]

const EMITTED_REGISTERS = [
  'x0',
  'x1',
  'x2',
  'x3',
  'x4',
  'x8',
  'x9',
  'x29',
  'x30',
  'sp',
  'w0',
]

describe('arm64TokenDoc', () => {
  it('documents every emitted mnemonic', () => {
    for (const mnemonic of ARM64_MNEMONICS) {
      const doc = arm64TokenDoc(mnemonic)
      expect(doc, mnemonic).not.toBeNull()
      expect(doc?.title, mnemonic).toContain(mnemonic)
    }
  })

  it('documents every emitted runtime call by name', () => {
    for (const name of EMITTED_RUNTIME_CALLS) {
      const doc = arm64TokenDoc(name)
      expect(doc, name).not.toBeNull()
      expect(doc?.title, name).toContain(name)
    }
  })

  it('falls back to a generic entry for unknown rt_ symbols', () => {
    expect(arm64TokenDoc('rt_next_year_feature')?.title).toBe(
      'rt_* — Monkey runtime call'
    )
  })

  it('documents every emitted directive and register', () => {
    for (const token of [...EMITTED_DIRECTIVES, ...EMITTED_REGISTERS]) {
      const doc = arm64TokenDoc(token)
      expect(doc, token).not.toBeNull()
      expect(doc?.title, token).toContain(token)
    }
  })

  it('documents registers beyond the named set generically', () => {
    expect(arm64TokenDoc('x12')?.title).toBe('x12 — general-purpose register')
    expect(arm64TokenDoc('w7')?.title).toBe('w7 — low half of x7')
  })

  it.each([
    ['main', 'main — program entry point'],
    ['g_globals', 'g_globals — global variable slots'],
    ['.Lmain_exit', '.Lmain_exit — end of main'],
    ['.L0', '.L0 — local label'],
    ['.L42', '.L42 — local label'],
    ['.Lfn0', '.Lfn0 — compiled Monkey function'],
    ['.Lfn3_ret', '.Lfn3_ret — function epilogue'],
    ['.Lstr2', '.Lstr2 — string literal'],
    ['lsl', 'lsl — logical shift left'],
    ['lo12', ':lo12: — low 12 bits of an address'],
  ])('documents %s', (token, title) => {
    expect(arm64TokenDoc(token)?.title).toBe(title)
  })

  it.each(['foo', '0x2', '16', '', 'xzr', 'monkey', '.Lweird'])(
    'has nothing to say about %j',
    (token) => {
      expect(arm64TokenDoc(token)).toBeNull()
    }
  )
})

describe('arm64TokenAt', () => {
  const line = '    movz x0, #0x2                   // 1'

  it('finds the token containing the column', () => {
    expect(arm64TokenAt(line, 4)).toEqual({ from: 4, to: 8, text: 'movz' })
    expect(arm64TokenAt(line, 6)).toEqual({ from: 4, to: 8, text: 'movz' })
    expect(arm64TokenAt(line, 9)).toEqual({ from: 9, to: 11, text: 'x0' })
  })

  it('leans left when the column sits just past a token', () => {
    expect(arm64TokenAt(line, 8)).toEqual({ from: 4, to: 8, text: 'movz' })
    expect(arm64TokenAt('ret', 3)).toEqual({ from: 0, to: 3, text: 'ret' })
  })

  it('returns null over whitespace and punctuation', () => {
    expect(arm64TokenAt(line, 2)).toBeNull()
    expect(arm64TokenAt('    str x0, [sp, #-16]!', 12)).toBeNull()
    expect(arm64TokenAt('', 0)).toBeNull()
  })

  it('splits :lo12:symbol into relocation and symbol', () => {
    const address = '    add x0, x0, :lo12:g_globals'
    expect(arm64TokenAt(address, 17)).toEqual({
      from: 17,
      to: 21,
      text: 'lo12',
    })
    expect(arm64TokenAt(address, 23)).toEqual({
      from: 22,
      to: 31,
      text: 'g_globals',
    })
  })

  it('keeps dotted labels whole', () => {
    expect(arm64TokenAt('    b .L0', 7)).toEqual({ from: 6, to: 9, text: '.L0' })
    expect(arm64TokenAt('main:', 0)).toEqual({ from: 0, to: 4, text: 'main' })
  })

  it('does not expose instruction-like words inside comments', () => {
    const trailing = '    movz x0, #0x2              // let add = 1'
    expect(arm64TokenAt(trailing, trailing.lastIndexOf('add'))).toBeNull()
    expect(arm64TokenAt('// bl rt_call', 3)).toBeNull()
    expect(arm64TokenAt(trailing, trailing.indexOf('movz'))?.text).toBe('movz')
  })
})
