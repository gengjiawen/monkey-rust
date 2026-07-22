const assert = require('node:assert/strict')

const { minify, parseProgram } = require('..')

assert.equal(minify('let longName = 40 + 2; longName;').code, '42;')
assert.equal(parseProgram('9007199254740993').body[0].raw, '9007199254740993')
