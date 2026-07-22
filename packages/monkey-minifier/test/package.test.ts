import { readFileSync } from 'node:fs'

import { describe, expect, it } from 'vitest'

type PackageJson = {
  version: string
  dependencies: Record<string, string>
}

function readPackageJson(url: URL) {
  return JSON.parse(readFileSync(url, 'utf8')) as PackageJson
}

describe('package metadata', () => {
  it('keeps the minifier on the same release line as its Wasm API', () => {
    const rootPackage = readPackageJson(
      new URL('../../../package.json', import.meta.url)
    )
    const minifierPackage = readPackageJson(
      new URL('../package.json', import.meta.url)
    )
    const playgroundPackage = readPackageJson(
      new URL('../../playground/package.json', import.meta.url)
    )
    const wasmRange = minifierPackage.dependencies['@gengjiawen/monkey-wasm']

    expect(minifierPackage.version).toBe(rootPackage.version)
    expect(wasmRange.replace(/^workspace:/, '')).toBe(
      `^${minifierPackage.version}`
    )
    expect(playgroundPackage.dependencies['@gengjiawen/monkey-minifier']).toBe(
      `workspace:^${minifierPackage.version}`
    )
  })
})
