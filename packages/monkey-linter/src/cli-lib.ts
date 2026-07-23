import { readFileSync } from 'node:fs'

import { lint } from './node'
import type { Diagnostic, LintOptions, RuleLevel } from './types'

/**
 * The CLI's logic, kept out of the `bin` entry (`cli.ts`) so every piece —
 * flag parsing, byte-offset location math, output formatting, exit codes —
 * is a plain exported function tests can call without spawning a process.
 */

export interface CliOptions {
  format: 'pretty' | 'json'
  denyWarnings: boolean
  rules: Record<string, RuleLevel>
  files: string[]
}

export interface FileResult {
  file: string
  source: string
  diagnostics: Diagnostic[]
}

export interface SourceLocation {
  line: number
  column: number
}

export interface SourceIndex {
  /** Map a UTF-8 byte offset to a 1-based line and (character) column. */
  locate(byteOffset: number): SourceLocation
  /** The text of a 1-based line, without its trailing newline. */
  lineText(line: number): string
}

const RULE_LEVELS = new Set<RuleLevel>(['off', 'warn', 'error'])
const ENCODER = new TextEncoder()
const DECODER = new TextDecoder()

export const HELP = `Usage: monkey-lint [options] [file ...]

Lint Monkey source files. With no files, source is read from stdin.

Options:
  --format <pretty|json>   Output format (default: pretty)
  --rule <name>:<level>    Override a rule's level: off | warn | error
                           (repeatable)
  --deny-warnings          Exit with code 1 if any warning is reported
  -h, --help               Show this help

Exit codes:
  0  no problems (or only warnings without --deny-warnings)
  1  at least one error, or any warning with --deny-warnings
`

export function parseArgs(argv: string[]): CliOptions | 'help' {
  const options: CliOptions = {
    format: 'pretty',
    denyWarnings: false,
    rules: {},
    files: [],
  }
  for (let index = 0; index < argv.length; index++) {
    let flag = argv[index]
    let inline: string | undefined
    if (flag.startsWith('--') && flag.includes('=')) {
      const eq = flag.indexOf('=')
      inline = flag.slice(eq + 1)
      flag = flag.slice(0, eq)
    }
    const takeValue = (): string => {
      const value = inline ?? argv[++index]
      if (value === undefined) {
        throw new Error(`${flag} expects a value`)
      }
      return value
    }
    switch (flag) {
      case '-h':
      case '--help':
        return 'help'
      case '--deny-warnings':
        options.denyWarnings = true
        break
      case '--format': {
        const value = takeValue()
        if (value !== 'pretty' && value !== 'json') {
          throw new Error(`--format expects 'pretty' or 'json', got '${value}'`)
        }
        options.format = value
        break
      }
      case '--rule': {
        const value = takeValue()
        const separator = value.lastIndexOf(':')
        if (separator <= 0) {
          throw new Error(`--rule expects <name>:<level>, got '${value}'`)
        }
        const name = value.slice(0, separator)
        const level = value.slice(separator + 1) as RuleLevel
        if (!RULE_LEVELS.has(level)) {
          throw new Error(
            `--rule level must be off, warn, or error, got '${level}'`
          )
        }
        options.rules[name] = level
        break
      }
      default:
        if (flag.startsWith('-')) {
          throw new Error(`unknown option: ${flag}`)
        }
        options.files.push(flag)
    }
  }
  return options
}

export function indexSource(source: string): SourceIndex {
  const bytes = ENCODER.encode(source)
  const lineStarts = [0]
  for (let i = 0; i < bytes.length; i++) {
    if (bytes[i] === 0x0a) {
      lineStarts.push(i + 1)
    }
  }
  const locate = (byteOffset: number): SourceLocation => {
    const clamped = Math.max(0, Math.min(byteOffset, bytes.length))
    let low = 0
    let high = lineStarts.length - 1
    while (low < high) {
      const mid = (low + high + 1) >> 1
      if (lineStarts[mid] <= clamped) {
        low = mid
      } else {
        high = mid - 1
      }
    }
    const column =
      DECODER.decode(bytes.subarray(lineStarts[low], clamped)).length + 1
    return { line: low + 1, column }
  }
  const lineText = (line: number): string => {
    if (line < 1 || line > lineStarts.length) {
      return ''
    }
    const start = lineStarts[line - 1]
    const end = line < lineStarts.length ? lineStarts[line] - 1 : bytes.length
    const text = DECODER.decode(bytes.subarray(start, end))
    return text.endsWith('\r') ? text.slice(0, -1) : text
  }
  return { locate, lineText }
}

/**
 * `file:line:col: severity rule: message`, followed by the offending source
 * line with the span underlined. Columns count characters, so the caret stays
 * aligned for multi-byte source. Diagnostics without a span (parse errors)
 * print the header line alone.
 */
export function formatPretty(results: FileResult[]): string {
  const lines: string[] = []
  for (const { file, source, diagnostics } of results) {
    const index = indexSource(source)
    for (const diagnostic of diagnostics) {
      const severity = diagnostic.severity === 'error' ? 'error' : 'warning'
      if (!diagnostic.span) {
        lines.push(
          `${file}: ${severity} ${diagnostic.rule}: ${diagnostic.message}`
        )
        continue
      }
      const start = index.locate(diagnostic.span.start)
      const end = index.locate(diagnostic.span.end)
      lines.push(
        `${file}:${start.line}:${start.column}: ${severity} ${diagnostic.rule}: ${diagnostic.message}`
      )
      // Tabs render at unpredictable widths; one space keeps the caret aligned.
      const text = index.lineText(start.line).replace(/\t/g, ' ')
      const width =
        end.line === start.line
          ? Math.max(1, end.column - start.column)
          : Math.max(1, text.length - start.column + 1)
      lines.push(`  ${text}`)
      lines.push(`  ${' '.repeat(start.column - 1)}${'^'.repeat(width)}`)
    }
  }
  return lines.length ? `${lines.join('\n')}\n` : ''
}

export function formatJson(results: FileResult[]): string {
  const payload = results.map(({ file, source, diagnostics }) => {
    const { locate } = indexSource(source)
    return {
      file,
      diagnostics: diagnostics.map((diagnostic) => ({
        rule: diagnostic.rule,
        severity: diagnostic.severity,
        message: diagnostic.message,
        ...(diagnostic.span
          ? {
              span: diagnostic.span,
              location: {
                start: locate(diagnostic.span.start),
                end: locate(diagnostic.span.end),
              },
            }
          : {}),
      })),
    }
  })
  return `${JSON.stringify(payload, null, 2)}\n`
}

/** 1 when any error is present, or any warning under `--deny-warnings`. */
export function exitCodeFor(
  results: FileResult[],
  denyWarnings: boolean
): number {
  let hasError = false
  let hasWarning = false
  for (const { diagnostics } of results) {
    for (const diagnostic of diagnostics) {
      if (diagnostic.severity === 'error') {
        hasError = true
      } else {
        hasWarning = true
      }
    }
  }
  if (hasError) {
    return 1
  }
  if (denyWarnings && hasWarning) {
    return 1
  }
  return 0
}

/**
 * Run the CLI against `argv` and return what to print and how to exit,
 * leaving the actual writing to the `bin` shell.
 */
export function runCli(argv: string[]): { output: string; exitCode: number } {
  const parsed = parseArgs(argv)
  if (parsed === 'help') {
    return { output: HELP, exitCode: 0 }
  }
  const lintOptions: LintOptions = { rules: parsed.rules }
  const inputs = parsed.files.length
    ? parsed.files.map((file) => ({
        file,
        source: readFileSync(file, 'utf8'),
      }))
    : [{ file: '<stdin>', source: readFileSync(0, 'utf8') }]

  const results: FileResult[] = inputs.map(({ file, source }) => ({
    file,
    source,
    diagnostics: lint(source, lintOptions).diagnostics,
  }))

  const output =
    parsed.format === 'json' ? formatJson(results) : formatPretty(results)
  return { output, exitCode: exitCodeFor(results, parsed.denyWarnings) }
}
