import { describe, expect, it } from 'vitest'

import { monkeyLintDiagnostics } from '../lint'

describe('monkeyLintDiagnostics', () => {
  it('maps UTF-8 byte spans onto UTF-16 editor positions', async () => {
    // The two-character string literal is 6 UTF-8 bytes but 2 UTF-16 units, so
    // the `len()` span only lines up in the editor after conversion.
    const diagnostics = await monkeyLintDiagnostics('let s = "你好"; len();')

    expect(diagnostics).toEqual([
      {
        from: 4,
        to: 5,
        severity: 'warning',
        source: 'no-unused-let',
        message: "'s' is declared but never used",
      },
      {
        from: 14,
        to: 19,
        severity: 'error',
        source: 'builtin-arity',
        message: "builtin 'len' expects 1 argument, got 0",
      },
    ])
  })

  it('anchors a span-less parser diagnostic at the document start', async () => {
    const diagnostics = await monkeyLintDiagnostics('let x = 1 +')

    expect(diagnostics).toEqual([
      expect.objectContaining({
        from: 0,
        to: 0,
        severity: 'error',
        source: 'parse-error',
      }),
    ])
  })

  it('returns nothing for a clean document', async () => {
    expect(await monkeyLintDiagnostics('puts(1);')).toEqual([])
    expect(await monkeyLintDiagnostics('')).toEqual([])
  })
})
