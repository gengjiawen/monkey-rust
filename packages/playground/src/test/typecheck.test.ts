import { diagnosticCount } from '@codemirror/lint'
import { EditorView } from '@codemirror/view'
import { describe, expect, it, vi } from 'vitest'

import { monkeyTypeDiagnostics, runMonkeyTypeCheck } from '../typecheck'

describe('monkeyTypeDiagnostics', () => {
  it('maps UTF-8 byte spans onto UTF-16 editor positions', async () => {
    // The greeting is 6 UTF-8 bytes but 2 UTF-16 units, so the offending
    // argument only lines up in the editor after conversion.
    const diagnostics = await monkeyTypeDiagnostics(
      'let s = "你好"; let f = fn(n: int) { n }; f(s);'
    )

    expect(diagnostics).toEqual([
      {
        from: 42,
        to: 43,
        severity: 'error',
        source: 'type-mismatch',
        message: "type 'string' is not assignable to type 'int'",
      },
    ])
  })

  it('reports a parse failure through the same channel', async () => {
    const diagnostics = await monkeyTypeDiagnostics('let x: = 1;')

    expect(diagnostics).toEqual([
      expect.objectContaining({ severity: 'error', source: 'parse-error' }),
    ])
  })

  it('returns nothing for a clean document', async () => {
    expect(await monkeyTypeDiagnostics('let n: int = 1; puts(n);')).toEqual([])
    expect(await monkeyTypeDiagnostics('')).toEqual([])
  })

  it('stays quiet on an unannotated program', async () => {
    expect(
      await monkeyTypeDiagnostics('let add = fn(a, b) { a + b }; add(1, "x");')
    ).toEqual([])
  })
})

describe('runMonkeyTypeCheck', () => {
  it('attaches diagnostics to the view and opens the panel', async () => {
    const view = new EditorView({
      doc: 'let n: int = "x";',
      parent: document.body,
    })
    try {
      await runMonkeyTypeCheck(view)

      expect(diagnosticCount(view.state)).toBe(1)
      expect(view.dom.querySelector('.cm-panel-lint')).not.toBeNull()
      expect(view.dom.textContent).toContain(
        "type 'string' is not assignable to type 'int'"
      )
    } finally {
      view.destroy()
    }
  })

  it('still opens the panel for a clean document', async () => {
    const view = new EditorView({ doc: 'puts(1);', parent: document.body })
    try {
      await runMonkeyTypeCheck(view)

      expect(diagnosticCount(view.state)).toBe(0)
      expect(view.dom.querySelector('.cm-panel-lint')).not.toBeNull()
    } finally {
      view.destroy()
    }
  })

  it('discards diagnostics when the document changes during the run', async () => {
    let resolveDiagnostics!: (
      diagnostics: Awaited<ReturnType<typeof monkeyTypeDiagnostics>>
    ) => void
    const diagnostics = new Promise<
      Awaited<ReturnType<typeof monkeyTypeDiagnostics>>
    >((resolve) => {
      resolveDiagnostics = resolve
    })
    const view = new EditorView({
      doc: 'let n: int = "x";',
      parent: document.body,
    })
    try {
      const run = runMonkeyTypeCheck(view, () => diagnostics)
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: 'puts(1);' },
      })
      resolveDiagnostics([
        {
          from: 13,
          to: 16,
          severity: 'error',
          source: 'type-mismatch',
          message: "type 'string' is not assignable to type 'int'",
        },
      ])
      await run

      expect(diagnosticCount(view.state)).toBe(0)
      expect(view.dom.querySelector('.cm-panel-lint')).toBeNull()
    } finally {
      view.destroy()
    }
  })

  it('shows a diagnostic when the checker fails', async () => {
    const error = new Error('chunk load failed')
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {})
    const view = new EditorView({ doc: 'puts(1);', parent: document.body })
    try {
      await runMonkeyTypeCheck(view, async () => {
        throw error
      })

      expect(consoleError).toHaveBeenCalledWith(
        'monkey-typecheck failed:',
        error
      )
      expect(diagnosticCount(view.state)).toBe(1)
      expect(view.dom.querySelector('.cm-panel-lint')).not.toBeNull()
      expect(view.dom.textContent).toContain(
        'Type checker failed: chunk load failed'
      )
    } finally {
      consoleError.mockRestore()
      view.destroy()
    }
  })
})
