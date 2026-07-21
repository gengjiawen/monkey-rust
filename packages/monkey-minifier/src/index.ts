import { parse_lossless } from '@gengjiawen/monkey-wasm'

import {
  minifyWithParser,
  parseProgramWithParser,
  type MinifyOptions,
  type MinifyResult,
} from './core'
import type { Program } from './types'

export function minify(
  source: string,
  options: MinifyOptions = {}
): MinifyResult {
  return minifyWithParser(parse_lossless, source, options)
}

export function parseProgram(source: string): Program {
  return parseProgramWithParser(parse_lossless, source)
}

export { eliminateDeadLets, foldConstants } from './fold'
export { mangle } from './mangle'
export { printExpression, printProgram } from './printer'
export type { MinifyOptions, MinifyResult } from './core'
export type { MangleOptions } from './mangle'
export type * from './types'
