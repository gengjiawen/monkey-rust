// The linter, minifier and type checker each need the wasm module, and in Node
// they all reach it through the same generated glue module. The glue keeps the
// live `WebAssembly.Instance` in module-level state, so a second instantiation
// used to rebind it and leave whoever loaded first reading a memory buffer that
// was no longer its own. Nothing inside a single package can catch that; this
// script loads all three in one process and checks they still agree.
//
// Run from the repo root, after the three packages have been built.

const assert = require('node:assert/strict')
const { readFileSync } = require('node:fs')
const path = require('node:path')

const packages = path.join(__dirname, '..', 'packages')
const { lint } = require(path.join(packages, 'monkey-linter'))
const { check } = require(path.join(packages, 'monkey-typechecker'))
const { minify } = require(path.join(packages, 'monkey-minifier'))

const source = 'let unused = 1; puts("hi");\n'

// Interleaved, because the failure only showed on the call *after* another
// package instantiated: each entry has to keep working once the others exist.
const rounds = [
  ['lint', () => lint(source)],
  ['check', () => check(source)],
  ['minify', () => minify(source)],
  ['lint', () => lint(source)],
  ['check', () => check(source)],
  ['minify', () => minify(source)],
]

const results = new Map()

for (const [name, run] of rounds) {
  let result
  try {
    result = run()
  } catch (error) {
    throw new Error(`${name}() threw after another package loaded: ${error.message}`)
  }

  const previous = results.get(name)
  if (previous === undefined) {
    results.set(name, JSON.stringify(result))
  } else {
    assert.equal(
      JSON.stringify(result),
      previous,
      `${name}() changed its answer after another package loaded`
    )
  }
}

// Every package still understands the source rather than failing to decode it.
assert.deepEqual(
  lint(source).diagnostics.map((diagnostic) => diagnostic.rule),
  ['no-unused-let']
)
assert.deepEqual(check(source).diagnostics, [])
assert.equal(minify(source).code, 'puts("hi");')

// One instance, shared: the registry the packages agree on has a single entry.
const registry = globalThis[Symbol.for('@gengjiawen/monkey-wasm.node-glue')]
assert.ok(registry instanceof Map, 'no shared wasm registry on globalThis')
assert.equal(registry.size, 1, `expected one wasm instance, got ${registry.size}`)

// The loader itself is copied into each package, because they publish
// independently and share no module that could carry it. Keep the copies
// identical, so a fix in one is a fix in all three.
const loaders = ['monkey-linter', 'monkey-minifier', 'monkey-typechecker'].map(
  (name) => readFileSync(path.join(packages, name, 'src', 'wasm-node.ts'), 'utf8')
)
assert.ok(
  loaders.every((loader) => loader === loaders[0]),
  'packages/*/src/wasm-node.ts have drifted apart'
)

console.log('analyzer packages coexist in one process')
