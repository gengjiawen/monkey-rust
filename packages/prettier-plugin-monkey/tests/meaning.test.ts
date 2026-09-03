import { describe, expect, it } from 'vitest'
import prettier from 'prettier'
import { run_gc_with_report } from '@gengjiawen/monkey-wasm'
import * as plugin from '../src/index'
import { parse } from '../src/parser'

async function format(code: string, options = {}) {
  return await prettier.format(code, {
    parser: 'monkey',
    plugins: [plugin],
    ...options,
  })
}

/** What the program does, reduced to the part formatting must not change. */
function evaluate(source: string) {
  const envelope = JSON.parse(run_gc_with_report(source))
  return envelope.status === 'ok'
    ? { status: 'ok', result: envelope.result }
    : { status: 'error', message: envelope.message }
}

// Every entry is a program the formatter used to rewrite into a different one.
const programs: [name: string, source: string, options?: object][] = [
  [
    'an expression statement followed by an array literal',
    `let a = [1, 2];
a;
[0];
`,
  ],
  [
    'an expression statement followed by a parenthesized expression',
    `let a = 1;
let b = 2;
let c = 3;
puts(a);
(a + b) * c;
`,
  ],
  [
    'expression statements inside a block',
    `let f = fn() {
  [1, 2];
  [0];
};
f();
`,
  ],
  [
    'an if expression indexed by the next line',
    `if (true) { [1, 2] } else { [3] }
[0];
`,
  ],
  [
    'an array that has to break',
    `let x = [1, 2, 3, 4];
len(x);
`,
    { printWidth: 20, trailingComma: 'all' },
  ],
  [
    'an integer literal past 2^53',
    `let x = 9223372036854775807;
x;
`,
  ],
  [
    'the smallest integer literal',
    `let x = -9223372036854775807;
x;
`,
  ],
  [
    'a comment after a multi-byte string',
    `let s = "éé";
// c
let t = 1;
t;
`,
  ],
  [
    'a comment inside a block after a multi-byte string',
    `let s = "日本語";
let f = fn() {
  // inner
  1;
};
f();
`,
  ],
]

describe('formatting preserves meaning', () => {
  it.each(programs)('%s', async (_name, source, options = {}) => {
    const formatted = await format(source, options)

    expect(() => parse(formatted, {})).not.toThrow()
    expect(await format(formatted, options)).toBe(formatted)
    expect(evaluate(formatted)).toEqual(evaluate(source))
  })
})

describe('the pieces that used to break', () => {
  it('terminates expression statements', async () => {
    expect(await format('let a = [1, 2];\na;\n[0];\n')).toBe(
      'let a = [1, 2];\na;\n[0];\n'
    )
  })

  it('keeps integer literals as they were written', async () => {
    expect(await format('let x=9223372036854775807;')).toBe(
      'let x = 9223372036854775807;\n'
    )
  })

  it('places comments correctly after multi-byte characters', async () => {
    expect(await format('let s = "éé";\n// c\nlet t = 1;\n')).toBe(
      'let s = "éé";\n// c\nlet t = 1;\n'
    )
  })
})
