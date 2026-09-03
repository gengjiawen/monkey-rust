import { readFileSync } from 'node:fs'

/**
 * Node's loader for the bundled wasm module, shared by every package that needs
 * one.
 *
 * This file is deliberately identical in `monkey-linter`, `monkey-minifier` and
 * `monkey-typechecker`: they publish independently and have no module in common
 * apart from `@gengjiawen/monkey-wasm` itself, which is generated and so cannot
 * carry it. `scripts/analyzers-coexist.cjs` is the regression test.
 */

const GLUE = '@gengjiawen/monkey-wasm/monkey_wasm_bg.js'
const BINARY = '@gengjiawen/monkey-wasm/monkey_wasm_bg.wasm'

/**
 * Where the shared instances live. A well-known symbol on `globalThis` is the
 * only place three independent packages can agree on.
 */
const REGISTRY = Symbol.for('@gengjiawen/monkey-wasm.node-glue')

export interface MonkeyWasmGlue extends WebAssembly.ModuleImports {
  __wbg_set_wasm(exports: WebAssembly.Exports): void
}

type GlueRegistry = Map<string, MonkeyWasmGlue>

function registry(): GlueRegistry {
  const host = globalThis as typeof globalThis & {
    [REGISTRY]?: GlueRegistry
  }
  return (host[REGISTRY] ??= new Map())
}

/**
 * The glue module for `@gengjiawen/monkey-wasm`, bound to its wasm instance.
 *
 * The glue is one module instance in Node's cache and it keeps the live
 * `WebAssembly.Instance` in module-level state (`wasm`,
 * `cachedUint8ArrayMemory0`, ...). Instantiating a second time rebinds that
 * state, leaving whoever instantiated first reading a memory buffer that is no
 * longer its own — every subsequent call fails with `The encoded data was not
 * valid for encoding utf-8`. So there is exactly one instantiation per glue
 * module, and the cache is keyed by where the glue resolved to: two
 * installations of the package are two modules, each of which needs its own.
 */
export function loadMonkeyWasm(): MonkeyWasmGlue {
  const gluePath = require.resolve(GLUE)
  const instances = registry()
  const cached = instances.get(gluePath)
  if (cached) {
    return cached
  }

  // wasm-pack's bundler target statically imports `.wasm`, which Node cannot
  // execute directly. Load the generated glue without its bundler entrypoint
  // and instantiate the same module through Node's WebAssembly API. Node 24 can
  // synchronously require this dependency's ESM glue module.
  const glue = require(GLUE) as MonkeyWasmGlue
  const module = new WebAssembly.Module(readFileSync(require.resolve(BINARY)))
  const instance = new WebAssembly.Instance(module, {
    './monkey_wasm_bg.js': glue,
  })
  glue.__wbg_set_wasm(instance.exports)
  const start = instance.exports.__wbindgen_start
  if (typeof start === 'function') {
    start()
  }

  instances.set(gluePath, glue)
  return glue
}
