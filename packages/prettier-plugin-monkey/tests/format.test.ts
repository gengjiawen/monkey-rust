import { describe, it, expect } from 'vitest';
import prettier from 'prettier';
import * as plugin from '../src/index';
import { parse } from '../src/parser';

async function format(code: string, options = {}) {
  return await prettier.format(code, {
    parser: 'monkey',
    plugins: [plugin],
    ...options,
  });
}

describe('Prettier Plugin Monkey', () => {
  it('formats let statements', async () => {
    const input = 'let   x=5;';
    const expected = 'let x = 5;\n';
    expect(await format(input)).toBe(expected);
  });

  it('formats return statements', async () => {
    const input = 'return   42;';
    const expected = 'return 42;\n';
    expect(await format(input)).toBe(expected);
  });

  it('formats binary expressions', async () => {
    const input = 'let x=1+2*3;';
    const expected = 'let x = 1 + (2 * 3);\n';
    expect(await format(input)).toBe(expected);
  });

  it('formats function declarations', async () => {
    const input = 'let add=fn(a,b){a+b};';
    const expected = `let add = fn(a, b) {
  a + b
};
`;
    expect(await format(input)).toBe(expected);
  });

  it('formats if expressions', async () => {
    const input = 'let x=if(true){1}else{2};';
    const expected = `let x = if (true) {
  1
} else {
  2
};
`;
    expect(await format(input)).toBe(expected);
  });

  it('formats arrays', async () => {
    const input = 'let arr=[1,2,3];';
    const expected = 'let arr = [1, 2, 3];\n';
    expect(await format(input)).toBe(expected);
  });

  it('formats long arrays with line breaks', async () => {
    const input = 'let arr=["aaaaaaaaaaaaaaaaaaaa","bbbbbbbbbbbbbbbbbbbb"];';
    const expected = `let arr = [
  "aaaaaaaaaaaaaaaaaaaa",
  "bbbbbbbbbbbbbbbbbbbb",
];
`;
    const output = await format(input, { printWidth: 20, trailingComma: 'all' });
    expect(output).toBe(expected);
  });

  it('formats hash literals with correct spacing and trailing comma behavior', async () => {
    const input = 'let h={"a":"aaaaaaaaaaaaaaaaaaaa","b":"bbbbbbbbbbbbbbbbbbbb"};';
    const expected = `let h = {
  "a": "aaaaaaaaaaaaaaaaaaaa",
  "b": "bbbbbbbbbbbbbbbbbbbb",
};
`;
    const output = await format(input, {
      printWidth: 20,
      trailingComma: 'all',
      bracketSpacing: true,
    });
    expect(output).toBe(expected);
  });

  it('respects bracketSpacing=false for single-line hash literals', async () => {
    const input = 'let h={"a":1,"b":2};';
    const expected = 'let h = {"a": 1, "b": 2};\n';
    const output = await format(input, { bracketSpacing: false });
    expect(output).toBe(expected);
  });

  it('formats function calls', async () => {
    const input = 'puts(len(arr));';
    const output = await format(input);
    expect(output).toContain('puts');
    expect(output).toContain('len');
  });

  it('formats index expressions', async () => {
    const input = 'let x=arr[0];';
    const expected = 'let x = (arr[0]);\n';
    expect(await format(input)).toBe(expected);
  });

  it('handles empty program', async () => {
    const input = '';
    const expected = '';
    expect(await format(input)).toBe(expected);
  });

  it('formats complex fibonacci example', async () => {
    const input = `let fibonacci=fn(x){if(x==0){0}else{if(x==1){return 1;}else{fibonacci(x-1)+fibonacci(x-2);}}};`;
    const output = await format(input);

    // Check that it's properly formatted with indentation
    expect(output).toContain('let fibonacci = fn(x)');
    expect(output).toContain('if (x == 0)');
    expect(output).toContain('return 1;');
  });

  it('keeps strings parseable even when singleQuote=true', async () => {
    const input = 'let name="it\'s";';
    const output = await format(input, { singleQuote: true });
    expect(output).toBe('let name = "it\'s";\n');
    expect(() => parse(output, {})).not.toThrow();
  });

  it('preserves line comments', async () => {
    const input = '// comment\nlet   x=1;';
    const expected = '// comment\nlet x = 1;\n';
    const output = await format(input);
    expect(output).toBe(expected);
  });

  it('is idempotent (formatting twice gives same result)', async () => {
    const input = 'let add=fn(a,b){a+b};';
    const firstFormat = await format(input);
    const secondFormat = await format(firstFormat);

    expect(firstFormat).toBe(secondFormat);
  });
});
