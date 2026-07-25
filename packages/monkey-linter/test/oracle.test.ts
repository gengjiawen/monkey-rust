import { run_gc_with_report } from '@gengjiawen/monkey-wasm'
import { describe, expect, it } from 'vitest'

import { rulesOf } from './helpers'

// The GC bytecode VM is the executable oracle: a construct is only worth
// flagging if the VM actually mishandles it. These tests pin each behavioural
// rule to the VM so a future runtime change that legalizes one of these
// constructs breaks the rule instead of silently drifting.

interface GcReport {
  status: 'ok' | 'error'
  stage?: string
  kind?: string
  message?: string
  result?: string
}

function runGc(source: string): GcReport {
  return JSON.parse(run_gc_with_report(source)) as GcReport
}

describe('no-literal-type-mismatch tracks the GC VM', () => {
  it.each(['1 + "a";', 'true + 1;', '"a" - "b";', '1 < "a";', 'true * false;'])(
    'flags %s, which the VM rejects with a type error',
    (source) => {
      expect(rulesOf(source)).toContain('no-literal-type-mismatch')
      const report = runGc(source)
      expect(report.status).toBe('error')
      expect(report.kind).toBe('type')
    }
  )

  it.each(['1 + 2;', '"a" + "b";', '1 < 2;'])(
    'stays quiet for %s, which the VM accepts',
    (source) => {
      expect(rulesOf(source)).not.toContain('no-literal-type-mismatch')
      expect(runGc(source).status).toBe('ok')
    }
  )
})

describe('builtin-arity tracks the GC VM', () => {
  it.each(['len();', 'len(1, 2);'])(
    'flags %s, which the VM evaluates to an arity error',
    (source) => {
      expect(rulesOf(source)).toContain('builtin-arity')
      // The VM surfaces a builtin arity violation as an `Error` result value
      // rather than halting, so the signal is the rendered result string.
      const report = runGc(source)
      expect(`${report.result ?? report.message ?? ''}`).toContain(
        'expected 1 argument'
      )
    }
  )

  it.each([
    ['first();', 'expected 1 argument'],
    ['first([1], [2]);', 'expected 1 argument'],
    ['last([1], [2]);', 'expected 1 argument'],
    ['rest([1], [2]);', 'expected 1 argument'],
    ['push([1]);', 'expected 2 arguments'],
    ['push([1], 2, 3);', 'expected 2 arguments'],
  ])('flags %s, which the VM evaluates to an arity error', (source, wording) => {
    expect(rulesOf(source)).toContain('builtin-arity')
    const report = runGc(source)
    expect(`${report.result ?? report.message ?? ''}`).toContain(wording)
  })

  it('stays quiet for len("hi"), which the VM accepts', () => {
    expect(rulesOf('len("hi");')).not.toContain('builtin-arity')
    expect(runGc('len("hi");').result).toBe('2')
  })

  it.each([
    ['first([1, 2]);', '1'],
    ['last([1, 2]);', '2'],
    ['push([1], 2);', '[1, 2]'],
  ])('stays quiet for %s, which the VM accepts', (source, expected) => {
    expect(rulesOf(source)).not.toContain('builtin-arity')
    expect(runGc(source).result).toBe(expected)
  })
})
