import { readFileSync } from 'node:fs'

import { describe, expect, it } from 'vitest'

type PackageJson = {
  version: string
  dependencies: Record<string, string>
}

function readPackageJson(url: URL): PackageJson {
  return JSON.parse(readFileSync(url, 'utf8')) as PackageJson
}

describe('package metadata', () => {
  it('keeps the linter on the same release line as its Wasm API', () => {
    const rootPackage = readPackageJson(
      new URL('../../../package.json', import.meta.url)
    )
    const linterPackage = readPackageJson(
      new URL('../package.json', import.meta.url)
    )
    const wasmRange = linterPackage.dependencies['@gengjiawen/monkey-wasm']

    expect(linterPackage.version).toBe(rootPackage.version)
    expect(wasmRange.replace(/^workspace:/, '')).toBe(
      `^${linterPackage.version}`
    )
  })
})
