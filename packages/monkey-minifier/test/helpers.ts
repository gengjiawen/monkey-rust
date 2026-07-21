import {
  compile_to_snapshot,
  parse_lossless,
  run_snapshot_with_output,
} from '@gengjiawen/monkey-wasm'

import type { Program } from '../src/types'

export function parseProgram(source: string): Program {
  const root = JSON.parse(parse_lossless(source)) as { Program: Program }
  return root.Program
}

export function canonical(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(canonical)
  }
  if (value !== null && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>)
        .filter(([key]) => key !== 'span' && key !== 'comments')
        .map(([key, child]) => [key, canonical(child)])
    )
  }
  return value
}

interface Observable {
  status: 'ok' | 'error'
  result?: string
  stage?: string
  kind?: string
  stdout: string
}

export function observe(source: string): Observable {
  const built = JSON.parse(compile_to_snapshot(source, false)) as {
    status: 'ok' | 'error'
    bytesHex?: string
    stage?: string
    kind?: string
  }
  if (built.status === 'error') {
    return {
      status: 'error',
      stage: built.stage,
      kind: built.kind,
      stdout: '',
    }
  }
  const bytes = Uint8Array.from(
    built.bytesHex!.match(/../g)!.map((byte) => Number.parseInt(byte, 16))
  )
  const run = JSON.parse(run_snapshot_with_output(bytes)) as Observable
  if (run.status === 'ok') {
    return { status: 'ok', result: run.result, stdout: run.stdout }
  }
  return {
    status: 'error',
    stage: run.stage,
    kind: run.kind,
    stdout: run.stdout,
  }
}
