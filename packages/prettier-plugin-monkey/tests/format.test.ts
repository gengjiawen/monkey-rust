import { describe, it, expect } from 'vitest'
import prettier from 'prettier'
import * as plugin from '../src/index'
import { parse } from '../src/parser'

async function format(code: string, options = {}) {
  return await prettier.format(code, {
    parser: 'monkey',
    plugins: [plugin],
    ...options,
  })
}

describe('Prettier Plugin Monkey', () => {
  it('formats let statements', async () => {
    const input = 'let   x=5;'
    const expected = 'let x = 5;\n'
    expect(await format(input)).toBe(expected)
  })

  it('formats return statements', async () => {
    const input = 'return   42;'
    const expected = 'return 42;\n'
    expect(await format(input)).toBe(expected)
  })

  it('formats debugger statements', async () => {
    const input = 'let x=1;debugger;let f=fn(){x;debugger;};'
    const expected = `let x = 1;
debugger;
let f = fn() {
  x
  debugger;
};
`
    const output = await format(input)
    expect(output).toBe(expected)
    expect(await format(output)).toBe(output)
  })

  it('formats binary expressions', async () => {
    const input = 'let x=1+2*3;'
    const expected = 'let x = 1 + (2 * 3);\n'
    expect(await format(input)).toBe(expected)
  })

  it('formats function declarations', async () => {
    const input = 'let add=fn(a,b){a+b};'
    const expected = `let add = fn(a, b) {
  a + b
};
`
    expect(await format(input)).toBe(expected)
  })

  it('formats if expressions', async () => {
    const input = 'let x=if(true){1}else{2};'
    const expected = `let x = if (true) {
  1
} else {
  2
};
`
    expect(await format(input)).toBe(expected)
  })

  it('formats arrays', async () => {
    const input = 'let arr=[1,2,3];'
    const expected = 'let arr = [1, 2, 3];\n'
    expect(await format(input)).toBe(expected)
  })

  it('formats long arrays with line breaks', async () => {
    const input = 'let arr=["aaaaaaaaaaaaaaaaaaaa","bbbbbbbbbbbbbbbbbbbb"];'
    const expected = `let arr = [
  "aaaaaaaaaaaaaaaaaaaa",
  "bbbbbbbbbbbbbbbbbbbb",
];
`
    const output = await format(input, {
      printWidth: 20,
      trailingComma: 'all',
    })
    expect(output).toBe(expected)
  })

  it('formats hash literals with correct spacing and trailing comma behavior', async () => {
    const input =
      'let h={"a":"aaaaaaaaaaaaaaaaaaaa","b":"bbbbbbbbbbbbbbbbbbbb"};'
    const expected = `let h = {
  "a": "aaaaaaaaaaaaaaaaaaaa",
  "b": "bbbbbbbbbbbbbbbbbbbb",
};
`
    const output = await format(input, {
      printWidth: 20,
      trailingComma: 'all',
      bracketSpacing: true,
    })
    expect(output).toBe(expected)
  })

  it('respects bracketSpacing=false for single-line hash literals', async () => {
    const input = 'let h={"a":1,"b":2};'
    const expected = 'let h = {"a": 1, "b": 2};\n'
    const output = await format(input, { bracketSpacing: false })
    expect(output).toBe(expected)
  })

  it('formats function calls', async () => {
    const input = 'puts(len(arr));'
    const output = await format(input)
    expect(output).toContain('puts')
    expect(output).toContain('len')
  })

  it('formats index expressions', async () => {
    const input = 'let x=arr[0];'
    const expected = 'let x = arr[0];\n'
    expect(await format(input)).toBe(expected)
  })

  it('formats class declarations and property assignments', async () => {
    const input = `class Node{constructor(value){this.value=value;}connect(other){this.next=other;}}`
    const expected = `class Node {
  constructor(value) {
    this.value = value;
  }

  connect(other) {
    this.next = other;
  }
}
`
    expect(await format(input)).toBe(expected)
  })

  it('formats this, new, and postfix chains without redundant parentheses', async () => {
    const input = `let result=new Node(1).connect(other).next[0];`
    const expected = 'let result = new Node(1).connect(other).next[0];\n'
    expect(await format(input)).toBe(expected)
  })

  it('parenthesizes low-precedence postfix children', async () => {
    expect(await format('let x=(a+b).value;')).toBe('let x = (a + b).value;\n')
    expect(await format('let x=(fn(){1})();')).toBe(
      `let x = (fn() {
  1
})();
`
    )
    expect(await format('let x=(-a).value;')).toBe('let x = (-a).value;\n')
  })

  it('wraps long method parameters and constructor arguments', async () => {
    const input = `class Example{method(firstParameter,secondParameter,thirdParameter){new Example(firstParameter,secondParameter,thirdParameter);}}`
    const expected = `class Example {
  method(
    firstParameter,
    secondParameter,
    thirdParameter
  ) {
    new Example(
      firstParameter,
      secondParameter,
      thirdParameter
    )
  }
}
`
    expect(await format(input, { printWidth: 30 })).toBe(expected)
  })

  it('preserves comments around and inside classes', async () => {
    const input = `// before
class Node {
  constructor() {
    // in constructor
  }
  // between methods
  value() { 1; }
}`
    const expected = `// before
class Node {
  constructor() {
    // in constructor
  }

  // between methods
  value() {
    1
  }
}
`
    expect(await format(input)).toBe(expected)
  })

  it('preserves a dangling comment in an empty class', async () => {
    const input = `class Empty {
  // still here
}`
    const expected = `class Empty {
  // still here
}
`
    expect(await format(input)).toBe(expected)
  })

  it('keeps formatted class programs parseable and idempotent', async () => {
    const input = `class Box{constructor(value){this.value=value;}reader(){fn(){this.value};}}let read=new Box(42).reader();read();`
    const firstFormat = await format(input)
    const secondFormat = await format(firstFormat)

    expect(() => parse(firstFormat, {})).not.toThrow()
    expect(firstFormat).toBe(secondFormat)
  })

  it('handles empty program', async () => {
    const input = ''
    const expected = ''
    expect(await format(input)).toBe(expected)
  })

  it('formats complex fibonacci example', async () => {
    const input = `let fibonacci=fn(x){if(x==0){0}else{if(x==1){return 1;}else{fibonacci(x-1)+fibonacci(x-2);}}};`
    const output = await format(input)

    // Check that it's properly formatted with indentation
    expect(output).toContain('let fibonacci = fn(x)')
    expect(output).toContain('if (x == 0)')
    expect(output).toContain('return 1;')
  })

  it('keeps strings parseable even when singleQuote=true', async () => {
    const input = 'let name="it\'s";'
    const output = await format(input, { singleQuote: true })
    expect(output).toBe('let name = "it\'s";\n')
    expect(() => parse(output, {})).not.toThrow()
  })

  it('preserves line comments', async () => {
    const input = '// comment\nlet   x=1;'
    const expected = '// comment\nlet x = 1;\n'
    const output = await format(input)
    expect(output).toBe(expected)
  })

  it('is idempotent (formatting twice gives same result)', async () => {
    const input = 'let add=fn(a,b){a+b};'
    const firstFormat = await format(input)
    const secondFormat = await format(firstFormat)

    expect(firstFormat).toBe(secondFormat)
  })

  // Unlike the minifier, the formatter preserves annotations: it normalizes the
  // spacing around `:` and leaves the type itself alone.
  it.each([
    ['let x:int=5;', 'let x: int = 5;\n'],
    ['let x  :  int?=5;', 'let x: int? = 5;\n'],
    ['let x:[int]=5;', 'let x: [int] = 5;\n'],
    ['let x:{string:[int]}=5;', 'let x: {string: [int]} = 5;\n'],
    ['let x:fn(int,string):bool=5;', 'let x: fn(int, string): bool = 5;\n'],
    ['let x:(fn(int):int)?=5;', 'let x: (fn(int): int)? = 5;\n'],
    ['let x:fn(int):int?=5;', 'let x: fn(int): int? = 5;\n'],
  ])('formats the annotation in %s', async (input, expected) => {
    expect(await format(input)).toBe(expected)
  })

  it('formats parameter and return type annotations', async () => {
    const input = 'let add=fn(a:int,b:[string]):bool{a};'
    const expected = `let add = fn(a: int, b: [string]): bool {
  a
};
`
    expect(await format(input)).toBe(expected)
  })

  it('formats method return types and leaves constructors bare', async () => {
    const input = `class Point{constructor(x:int,y:int){this.x=x;}norm():int{1;}}`
    const expected = `class Point {
  constructor(x: int, y: int) {
    this.x = x;
  }

  norm(): int {
    1
  }
}
`
    expect(await format(input)).toBe(expected)
  })

  it('keeps annotated programs parseable and idempotent', async () => {
    const input = `let box:{string:fn(int):int?}={"f":fn(n:int):int?{n}};let g:(fn():null)?=fn(a:[int],b):bool{a};`
    const firstFormat = await format(input)
    const secondFormat = await format(firstFormat)

    expect(() => parse(firstFormat, {})).not.toThrow()
    expect(firstFormat).toBe(secondFormat)
  })

  it.each([
    [
      `let xs: [
  // element
  int
] = [1];`,
      `let xs: [
  // element
  int
] = [1];
`,
    ],
    [
      `let f = fn(value: [
  // parameter element
  int
]) { value; };`,
      `let f = fn(
  value: [
    // parameter element
    int
  ]
) {
  value
};
`,
    ],
    [
      `let callback: fn(
  // input type
  int
): bool = fn(value) { value; };`,
      `let callback: fn(
  // input type
  int
): bool = fn(value) {
  value
};
`,
    ],
    [
      `let make = fn():
  // result type
  [int] { []; };`,
      `let make = fn():
  // result type
  [int] {
  []
};
`,
    ],
    [
      `let maybe: (fn(
  // optional input type
  int
): bool)? = null;`,
      `let maybe: (fn(
  // optional input type
  int
): bool)? = null;
`,
    ],
    [
      `let table: {
  // key type
  string:
  // value type
  [int]
} = {};`,
      `let table: {
  // key type
  string:
    // value type
    [int]
} = {};
`,
    ],
  ])('preserves comments inside type annotations', async (input, expected) => {
    const output = await format(input)

    expect(output).toBe(expected)
    expect(() => parse(output, {})).not.toThrow()
    expect(await format(output)).toBe(output)
  })

  it('wraps long annotated parameter lists', async () => {
    const input = `let f=fn(firstParameter:int,secondParameter:[string]):bool{firstParameter};`
    const expected = `let f = fn(
  firstParameter: int,
  secondParameter: [string]
): bool {
  firstParameter
};
`
    expect(await format(input, { printWidth: 30 })).toBe(expected)
  })
})
