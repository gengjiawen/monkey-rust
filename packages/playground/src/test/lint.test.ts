import { diagnosticCount } from '@codemirror/lint'
import { EditorView } from '@codemirror/view'
import { describe, expect, it, vi } from 'vitest'

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

  it('discards diagnostics when the document changes during the run', async () => {
    let resolveDiagnostics!: (
      diagnostics: Awaited<ReturnType<typeof monkeyLintDiagnostics>>
    ) => void
    const diagnostics = new Promise<
      Awaited<ReturnType<typeof monkeyLintDiagnostics>>
    >((resolve) => {
      resolveDiagnostics = resolve
    })
    const view = new EditorView({
      doc: 'let unused = 1;',
      parent: document.body,
    })
    try {
      const run = runMonkeyLint(view, () => diagnostics)
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: 'puts(1);' },
      })
      resolveDiagnostics([
        {
          from: 4,
          to: 10,
          severity: 'warning',
          source: 'no-unused-let',
          message: "'unused' is declared but never used",
        },
      ])
      await run

      expect(diagnosticCount(view.state)).toBe(0)
      expect(view.dom.querySelector('.cm-panel-lint')).toBeNull()
    } finally {
      view.destroy()
    }
  })

  it('shows a diagnostic when the linter fails', async () => {
    const error = new Error('chunk load failed')
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {})
    const view = new EditorView({ doc: 'puts(1);', parent: document.body })
    try {
      await runMonkeyLint(view, async () => {
        throw error
      })

      expect(consoleError).toHaveBeenCalledWith('monkey-lint failed:', error)
      expect(diagnosticCount(view.state)).toBe(1)
      expect(view.dom.querySelector('.cm-panel-lint')).not.toBeNull()
      expect(view.dom.textContent).toContain('Linter failed: chunk load failed')
    } finally {
      consoleError.mockRestore()
      view.destroy()
    }
  })
})
