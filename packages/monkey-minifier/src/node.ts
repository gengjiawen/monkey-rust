import { readFileSync } from 'node:fs'

import {
  minifyWithParser,
  parseProgramWithParser,
  type MinifyOptions,
  type MinifyResult,
  type ParseLossless,
} from './core'
import type { Program } from './types'

interface MonkeyWasmGlue extends WebAssembly.ModuleImports {
  __wbg_set_wasm(exports: WebAssembly.Exports): void
  parse_lossless: ParseLossless
}

function loadNodeParser(): ParseLossless {
  // wasm-pack's bundler target statically imports `.wasm`, which Node cannot
  // execute directly. Load the generated glue without its bundler entrypoint
  // and instantiate the same module through Node's WebAssembly API.
  // Node 24 can synchronously require this dependency's ESM glue module.
  const glue =
    require('@gengjiawen/monkey-wasm/monkey_wasm_bg.js') as MonkeyWasmGlue
  const wasmPath = require.resolve(
    '@gengjiawen/monkey-wasm/monkey_wasm_bg.wasm'
  )
  const module = new WebAssembly.Module(readFileSync(wasmPath))
  const instance = new WebAssembly.Instance(module, {
    './monkey_wasm_bg.js': glue,
  })
  glue.__wbg_set_wasm(instance.exports)
  const start = instance.exports.__wbindgen_start
  if (typeof start === 'function') {
    start()
  }
  return glue.parse_lossless
}

const parseLossless = loadNodeParser()

export function minify(
  source: string,
  options: MinifyOptions = {}
): MinifyResult {
  return minifyWithParser(parseLossless, source, options)
}

export function parseProgram(source: string): Program {
  return parseProgramWithParser(parseLossless, source)
}

export { eliminateDeadLets, foldConstants } from './fold'
export { mangle } from './mangle'
export { printExpression, printProgram } from './printer'
export type { MinifyOptions, MinifyResult } from './core'
export type { MangleOptions } from './mangle'
export type * from './types'
