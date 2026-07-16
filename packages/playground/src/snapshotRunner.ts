import { compile_to_snapshot, run_snapshot } from '@gengjiawen/monkey-wasm'

import {
  parseSnapshotBuildEnvelope,
  parseSnapshotRunEnvelope,
  type SnapshotBuildEnvelope,
  type SnapshotRunEnvelope,
} from './snapshot'

/**
 * Compile source into a `.mbc` snapshot plus its annotated layout.
 * Synchronous like `compile_with_debug`, so it can run in the debounced
 * compile pass. Parse/compile failures come back as envelope data.
 */
export function buildSnapshot(
  source: string,
  stripDebug: boolean
): SnapshotBuildEnvelope {
  return parseSnapshotBuildEnvelope(compile_to_snapshot(source, stripDebug))
}

/**
 * Execute `.mbc` bytes on the GC VM — the browser twin of
 * `monkey-gc run foo.mbc`. The bytes go through the validating snapshot
 * reader, not the parser or compiler.
 */
export async function runSnapshot(
  bytes: Uint8Array
): Promise<SnapshotRunEnvelope> {
  await Promise.resolve()
  return parseSnapshotRunEnvelope(run_snapshot(bytes))
}
