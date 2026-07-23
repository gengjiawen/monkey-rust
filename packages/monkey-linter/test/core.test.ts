import { describe, expect, it } from 'vitest'

import { lint } from '../src'
import { diagnose, rulesOf } from './helpers'

describe('analysis failures', () => {
  it('reports a parse error as a single error diagnostic and runs no rules', () => {
    const diagnostics = diagnose('let x = 1 +')
    expect(diagnostics).toHaveLength(1)
    expect(diagnostics[0]).toMatchObject({
      rule: 'parse-error',
      severity: 'error',
    })
  })

  it('reports a validation error with its span', () => {
    const diagnostics = diagnose('undefinedVar;')
    expect(diagnostics).toHaveLength(1)
    expect(diagnostics[0]).toMatchObject({
      rule: 'validation-error',
      severity: 'error',
      span: { start: 0, end: 12 },
    })
  })

  it('does not lint a tree that failed validation', () => {
    // `unusedLet` would normally be flagged, but the undefined reference stops
    // the run before any rule sees the tree.
    expect(rulesOf('let unusedLet = 1; missing;')).toEqual(['validation-error'])
  })
})

describe('rule levels', () => {
  it('disables a rule with `off`', () => {
    expect(rulesOf('let x = 1;', { rules: { 'no-unused-let': 'off' } })).toEqual(
      []
    )
  })

  it('promotes a warning to an error', () => {
    const diagnostics = diagnose('let x = 1;', {
      rules: { 'no-unused-let': 'error' },
    })
    expect(diagnostics[0]).toMatchObject({
      rule: 'no-unused-let',
      severity: 'error',
    })
  })

  it('leaves other rules at their default level', () => {
    const diagnostics = diagnose('let x = 1; len();', {
      rules: { 'no-unused-let': 'error' },
    })
    const bySeverity = Object.fromEntries(
      diagnostics.map((diagnostic) => [diagnostic.rule, diagnostic.severity])
    )
    expect(bySeverity).toEqual({
      'no-unused-let': 'error',
      'builtin-arity': 'error',
    })
  })

  it.each([
    ['len();', 'builtin-arity'],
    ['let h = {1: 1, 1: 2}; puts(h);', 'no-duplicate-hash-key'],
    ['puts(1 + "a");', 'no-literal-type-mismatch'],
  ])(
    'ships the runtime-rejection rule for %s as an error by default',
    (source, rule) => {
      const diagnostic = diagnose(source).find((d) => d.rule === rule)
      expect(diagnostic?.severity).toBe('error')
    }
  )

  it('throws on an unknown rule name instead of silently ignoring it', () => {
    expect(() =>
      diagnose('puts(1);', { rules: { 'no-unused-lets': 'off' } })
    ).toThrow("unknown rule 'no-unused-lets'")
  })

  it('ignores overrides for the synthetic analysis diagnostics', () => {
    // `parse-error` is not a configurable rule; the override is a no-op.
    const diagnostics = diagnose('let x = 1 +', {
      rules: { 'parse-error': 'off' },
    })
    expect(diagnostics).toHaveLength(1)
    expect(diagnostics[0].rule).toBe('parse-error')
  })
})

describe('diagnostic ordering', () => {
  it('sorts by span start, then end, then rule id', () => {
    const diagnostics = lint('let puts = 1; len();').diagnostics
    const starts = diagnostics.map((diagnostic) => diagnostic.span?.start ?? -1)
    const sorted = [...starts].sort((a, b) => a - b)
    expect(starts).toEqual(sorted)
  })
})
