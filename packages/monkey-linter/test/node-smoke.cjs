const assert = require('node:assert/strict')

const { lint } = require('..')

const flagged = lint('let unused = 1; puts("hi");')
assert.equal(flagged.diagnostics.length, 1)
assert.equal(flagged.diagnostics[0].rule, 'no-unused-let')
assert.equal(flagged.diagnostics[0].severity, 'warn')

assert.deepEqual(lint('puts("hi");').diagnostics, [])

const failed = lint('let x = 1 +')
assert.equal(failed.diagnostics[0].rule, 'parse-error')
assert.equal(failed.diagnostics[0].severity, 'error')
