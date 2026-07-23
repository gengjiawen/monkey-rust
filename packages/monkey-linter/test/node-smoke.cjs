const assert = require('node:assert/strict')
const { spawnSync } = require('node:child_process')
const path = require('node:path')

const { lint } = require('..')

const flagged = lint('let unused = 1; puts("hi");')
assert.equal(flagged.diagnostics.length, 1)
assert.equal(flagged.diagnostics[0].rule, 'no-unused-let')
assert.equal(flagged.diagnostics[0].severity, 'warn')

assert.deepEqual(lint('puts("hi");').diagnostics, [])

const failed = lint('let x = 1 +')
assert.equal(failed.diagnostics[0].rule, 'parse-error')
assert.equal(failed.diagnostics[0].severity, 'error')

// CLI end to end through the bin entry: `builtin-arity` violations are errors
// by default, so the exit code is 1, and pretty output underlines the span.
const cliPath = path.join(__dirname, '..', 'dist', 'cli.js')
const cli = spawnSync(process.execPath, [cliPath], {
  input: 'len(1, 2);\n',
  encoding: 'utf8',
})
assert.equal(cli.status, 1)
assert.match(cli.stdout, /^<stdin>:1:1: error builtin-arity: /m)
assert.match(cli.stdout, /^ {2}len\(1, 2\);$/m)
assert.match(cli.stdout, /^ {2}\^+$/m)

const help = spawnSync(process.execPath, [cliPath, '--help'], {
  encoding: 'utf8',
})
assert.equal(help.status, 0)
assert.match(help.stdout, /Usage: monkey-lint/)
