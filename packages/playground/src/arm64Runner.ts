import { compile_to_arm64 } from '@gengjiawen/monkey-wasm'

import { parseArm64BuildEnvelope, type Arm64BuildEnvelope } from './arm64'

/**
 * Lower Monkey source to AArch64 assembly with per-line source spans — the
 * browser twin of `monkey-asm emit`. Synchronous like `compile_with_debug`,
 * so it can run in the debounced compile pass. Parse/lowering failures come
 * back as envelope data.
 */
export function buildArm64(source: string): Arm64BuildEnvelope {
  return parseArm64BuildEnvelope(compile_to_arm64(source))
}
