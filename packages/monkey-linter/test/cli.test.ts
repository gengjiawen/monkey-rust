import { describe, expect, it } from 'vitest'

import {
  exitCodeFor,
  formatJson,
  formatPretty,
  indexSource,
  parseArgs,
  type FileResult,
} from '../src/cli-lib'
import type { Diagnostic } from '../src/types'

function fileResult(
  source: string,
  diagnostics: Diagnostic[],
  file = 'demo.monkey'
): FileResult {
  return { file, source, diagnostics }
}

describe('parseArgs', () => {
  it('defaults to pretty output on stdin', () => {
    expect(parseArgs([])).toEqual({
      format: 'pretty',
      denyWarnings: false,
      rules: {},
      files: [],
    })
  })

  it.each([['-h'], ['--help']])('recognizes %s', (flag) => {
    expect(parseArgs([flag])).toBe('help')
  })

  it('accepts --format as a pair or inline', () => {
    expect(parseArgs(['--format', 'json'])).toMatchObject({ format: 'json' })
    expect(parseArgs(['--format=json'])).toMatchObject({ format: 'json' })
  })

  it('collects repeated --rule overrides', () => {
    expect(
      parseArgs(['--rule', 'no-unused-let:off', '--rule=builtin-arity:warn'])
    ).toMatchObject({
      rules: { 'no-unused-let': 'off', 'builtin-arity': 'warn' },
    })
  })

  it('collects positional file arguments and flags together', () => {
    expect(parseArgs(['a.monkey', '--deny-warnings', 'b.monkey'])).toEqual({
      format: 'pretty',
      denyWarnings: true,
      rules: {},
      files: ['a.monkey', 'b.monkey'],
    })
  })

  it.each([
    [['--format', 'yaml'], /--format expects 'pretty' or 'json'/],
    [['--format'], /--format expects a value/],
    [['--rule', 'no-colon'], /--rule expects <name>:<level>/],
    [['--rule', 'no-unused-let:loud'], /--rule level must be off, warn, or error/],
    [['--verbose'], /unknown option: --verbose/],
  ])('rejects %j', (argv, error) => {
    expect(() => parseArgs(argv)).toThrow(error)
  })
})

describe('indexSource', () => {
  it('maps byte offsets to 1-based line and column', () => {
    const { locate } = indexSource('ab\ncd\n')
    expect(locate(0)).toEqual({ line: 1, column: 1 })
    expect(locate(3)).toEqual({ line: 2, column: 1 })
    expect(locate(4)).toEqual({ line: 2, column: 2 })
  })

  it('clamps out-of-range offsets', () => {
    const { locate } = indexSource('ab')
    expect(locate(-1)).toEqual({ line: 1, column: 1 })
    expect(locate(999)).toEqual({ line: 1, column: 3 })
  })

  it('counts columns in characters, not bytes', () => {
    // `名` occupies three UTF-8 bytes (offsets 4..7) but one column.
    const { locate } = indexSource('let 名 = 1;')
    expect(locate(4)).toEqual({ line: 1, column: 5 })
    expect(locate(7)).toEqual({ line: 1, column: 6 })
  })

  it('returns line text without terminators', () => {
    const { lineText } = indexSource('ab\r\ncd\nlast')
    expect(lineText(1)).toBe('ab')
    expect(lineText(2)).toBe('cd')
    expect(lineText(3)).toBe('last')
    expect(lineText(0)).toBe('')
    expect(lineText(4)).toBe('')
  })
})

describe('formatPretty', () => {
  it('prints location, severity, and an underlined source line', () => {
    const result = fileResult('let unused = 1; puts(2);\n', [
      {
        rule: 'no-unused-let',
        severity: 'warn',
        message: "'unused' is declared but never used",
        span: { start: 4, end: 10 },
      },
    ])
    expect(formatPretty([result])).toBe(
      "demo.monkey:1:5: warning no-unused-let: 'unused' is declared but never used\n" +
        '  let unused = 1; puts(2);\n' +
        '      ^^^^^^\n'
    )
  })

  it('keeps the caret aligned for multi-byte source', () => {
    const result = fileResult('let 名 = 1;\n', [
      {
        rule: 'no-unused-let',
        severity: 'warn',
        message: "'名' is declared but never used",
        span: { start: 4, end: 7 },
      },
    ])
    const lines = formatPretty([result]).split('\n')
    expect(lines[1]).toBe('  let 名 = 1;')
    expect(lines[2]).toBe('      ^')
  })

  it('prints span-less diagnostics as a bare header line', () => {
    const result = fileResult('let x = 1 +', [
      { rule: 'parse-error', severity: 'error', message: 'unexpected EOF' },
    ])
    expect(formatPretty([result])).toBe(
      'demo.monkey: error parse-error: unexpected EOF\n'
    )
  })

  it('prints nothing for clean results', () => {
    expect(formatPretty([fileResult('puts(1);', [])])).toBe('')
  })
})

describe('formatJson', () => {
  it('emits spans plus derived line/column locations', () => {
    const result = fileResult('let unused = 1; puts(2);\n', [
      {
        rule: 'no-unused-let',
        severity: 'warn',
        message: "'unused' is declared but never used",
        span: { start: 4, end: 10 },
      },
    ])
    expect(JSON.parse(formatJson([result]))).toEqual([
      {
        file: 'demo.monkey',
        diagnostics: [
          {
            rule: 'no-unused-let',
            severity: 'warn',
            message: "'unused' is declared but never used",
            span: { start: 4, end: 10 },
            location: {
              start: { line: 1, column: 5 },
              end: { line: 1, column: 11 },
            },
          },
        ],
      },
    ])
  })
})

describe('exitCodeFor', () => {
  const warning: Diagnostic = {
    rule: 'no-unused-let',
    severity: 'warn',
    message: 'w',
  }
  const error: Diagnostic = {
    rule: 'builtin-arity',
    severity: 'error',
    message: 'e',
  }

  it('is 0 for clean results', () => {
    expect(exitCodeFor([fileResult('', [])], false)).toBe(0)
  })

  it('is 0 for warnings unless --deny-warnings', () => {
    expect(exitCodeFor([fileResult('', [warning])], false)).toBe(0)
    expect(exitCodeFor([fileResult('', [warning])], true)).toBe(1)
  })

  it('is 1 whenever an error is present', () => {
    expect(exitCodeFor([fileResult('', [error])], false)).toBe(1)
    expect(exitCodeFor([fileResult('', []), fileResult('', [error])], false)).toBe(1)
  })
})
