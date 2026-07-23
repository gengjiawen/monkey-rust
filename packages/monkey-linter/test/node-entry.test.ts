import { describe, expect, it } from 'vitest'

import { lint } from '../src/node'

describe('Node entrypoint', () => {
  it('loads the bundler-target Wasm package without native Wasm imports', () => {
    const { diagnostics } = lint('let unused = 1; puts("hi");')
    expect(diagnostics).toEqual([
      {
        rule: 'no-unused-let',
        severity: 'warn',
        message: "'unused' is declared but never used",
        span: { start: 4, end: 10 },
      },
    ])
  })
})
