import { run_gc_with_report } from '@gengjiawen/monkey-wasm'

import { parseGcRunEnvelope, type GcRunEnvelope } from './gcReport'

export async function runGc(source: string): Promise<GcRunEnvelope> {
  await Promise.resolve()
  return parseGcRunEnvelope(run_gc_with_report(source))
}
