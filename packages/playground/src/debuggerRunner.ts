import { run_gc_with_debugger } from '@gengjiawen/monkey-wasm'

import {
  parseDebuggerRunEnvelope,
  type DebuggerRunEnvelope,
} from './debuggerReport'

export async function runDebugger(
  source: string
): Promise<DebuggerRunEnvelope> {
  await Promise.resolve()
  return parseDebuggerRunEnvelope(run_gc_with_debugger(source))
}
