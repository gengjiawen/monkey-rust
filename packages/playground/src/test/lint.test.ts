import { diagnosticCount } from '@codemirror/lint'
import { EditorView } from '@codemirror/view'
import { describe, expect, it } from 'vitest'

import { monkeyLintDiagnostics, runMonkeyLint } from '../lint'

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

describe('runMonkeyLint', () => {
  it('attaches diagnostics to the view and opens the panel', async () => {
    const view = new EditorView({
      doc: 'let unused = 1;',
      parent: document.body,
    })
    try {
      await runMonkeyLint(view)

      expect(diagnosticCount(view.state)).toBe(1)
      expect(view.dom.querySelector('.cm-panel-lint')).not.toBeNull()
      expect(view.dom.textContent).toContain(
        "'unused' is declared but never used"
      )
    } finally {
      view.destroy()
    }
  })

  it('still opens the panel for a clean document', async () => {
    const view = new EditorView({ doc: 'puts(1);', parent: document.body })
    try {
      await runMonkeyLint(view)

      expect(diagnosticCount(view.state)).toBe(0)
      expect(view.dom.querySelector('.cm-panel-lint')).not.toBeNull()
    } finally {
      view.destroy()
    }
  })
})
