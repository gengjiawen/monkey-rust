# prettier-plugin-monkey

A Prettier plugin for the [Monkey programming language](https://monkeylang.org/).

## Features

- 🚀 Fast parsing using WebAssembly (powered by [@gengjiawen/monkey-wasm](https://www.npmjs.com/package/@gengjiawen/monkey-wasm))
- 🎨 Automatic code formatting
- 🔧 Configurable formatting options
- 📦 Easy integration with existing Prettier setups

## Installation

```bash
npm install --save-dev prettier prettier-plugin-monkey
# or
yarn add -D prettier prettier-plugin-monkey
# or
pnpm add -D prettier prettier-plugin-monkey
```

## Usage

### With Prettier CLI

```bash
prettier --write "**/*.monkey"
```

### With `.prettierrc`

```json
{
  "plugins": ["prettier-plugin-monkey"],
  "printWidth": 80,
  "tabWidth": 2
}
```

### Programmatic API

```javascript
import prettier from 'prettier';

const code = `
let add = fn(a, b) {
a + b
};
`;

const formatted = await prettier.format(code, {
  parser: 'monkey',
  plugins: ['prettier-plugin-monkey'],
});

console.log(formatted);
```

## Supported File Extensions

- `.monkey`

## Configuration Options

This plugin supports all standard Prettier options:

- `printWidth` (default: 80)
- `tabWidth` (default: 2)
- `useTabs` (default: false)
- `trailingComma` (default: "none")
- `bracketSpacing` (default: true)

Notes:
- Monkey strings currently use double quotes only, so `singleQuote` is intentionally ignored.

## Examples

### Before

```monkey
let fibonacci=fn(x){if(x==0){0}else{if(x==1){return 1;}else{fibonacci(x-1)+fibonacci(x-2);}}};
```

### After

```monkey
let fibonacci = fn(x) {
  if (x == 0) {
    0
  } else {
    if (x == 1) {
      return 1;
    } else {
      fibonacci(x - 1) + fibonacci(x - 2);
    }
  }
};
```

## Development

```bash
# Install dependencies
npm install

# Build the plugin
npm run build

# Run tests
npm test

# Watch mode for development
npm run dev
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT

## Related Projects

- [monkey-rust](https://github.com/gengjiawen/monkey-rust) - Monkey interpreter in Rust
- [@gengjiawen/monkey-wasm](https://www.npmjs.com/package/@gengjiawen/monkey-wasm) - Monkey parser WASM package
- [Writing An Interpreter In Go](https://interpreterbook.com/) - The original Monkey language book
