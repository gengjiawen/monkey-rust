import {
  minifyWithParser,
  parseProgramWithParser,
  type MinifyOptions,
  type MinifyResult,
  type ParseLossless,
} from './core'
import type { Program } from './types'
import { loadMonkeyWasm, type MonkeyWasmGlue } from './wasm-node'

interface MonkeyParserGlue extends MonkeyWasmGlue {
  parse_lossless: ParseLossless
}

function loadNodeParser(): ParseLossless {
  return (loadMonkeyWasm() as MonkeyParserGlue).parse_lossless
}

let cachedParser: ParseLossless | undefined

/**
 * Minify Monkey source in Node, instantiating the bundled wasm module directly.
 * Instantiation happens on the first call and is cached, so merely importing
 * this module (e.g. for `monkey-minify --help`) never pays the wasm setup cost.
 */
export function minify(
  source: string,
  options: MinifyOptions = {}
): MinifyResult {
  cachedParser ??= loadNodeParser()
  return minifyWithParser(cachedParser, source, options)
}

export function parseProgram(source: string): Program {
  cachedParser ??= loadNodeParser()
  return parseProgramWithParser(cachedParser, source)
}

export { eliminateDeadLets, foldConstants } from './fold'
export { mangle } from './mangle'
export { printExpression, printProgram } from './printer'
export type { MinifyOptions, MinifyResult } from './core'
export type { MangleOptions } from './mangle'
export type * from './types'
