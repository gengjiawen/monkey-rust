# VS Code Extension 设计实现文档

> 本文说明 `packages/vscode-extension` 的设计目标、实现结构、WASM 集成、打包流程和维护策略。

## 目录

1. [概述](#1-概述)
2. [功能范围](#2-功能范围)
3. [项目结构](#3-项目结构)
4. [扩展 Manifest 设计](#4-扩展-manifest-设计)
5. [运行时架构](#5-运行时架构)
6. [WASM 加载策略](#6-wasm-加载策略)
7. [诊断与命令实现](#7-诊断与命令实现)
8. [语言资源](#8-语言资源)
9. [构建与打包](#9-构建与打包)
10. [版本同步与发布维护](#10-版本同步与发布维护)
11. [测试与验证](#11-测试与验证)
12. [后续演进](#12-后续演进)

---

## 1. 概述

`monkey-extension` 是 Monkey 语言的 VS Code 扩展，目标是把仓库已有的 Monkey 语言能力带到编辑器中，提供基础编辑体验和轻量交互能力。

扩展当前覆盖三类能力：

- 语言识别：让 VS Code 识别 `.monkey` 文件为 Monkey 语言。
- 编辑辅助：提供 TextMate 语法高亮、括号/注释配置和代码片段。
- 运行时能力：通过 `@gengjiawen/monkey-wasm` 提供解析诊断、AST 查看和字节码编译命令。

整体设计优先保持扩展轻量，不在 extension 包内重新构建 Rust/WASM，而是直接消费仓库内 `wasm/pkg` 生成的 `@gengjiawen/monkey-wasm` workspace 包，构建时由 esbuild 打进扩展 bundle。

---

## 2. 功能范围

### 2.1 已实现能力

| 能力       | 入口                                 | 说明                                         |
| ---------- | ------------------------------------ | -------------------------------------------- |
| 语言注册   | `package.json#contributes.languages` | 将 `.monkey` 文件关联到 `monkey` language id |
| 语法高亮   | `syntaxes/monkey.tmLanguage.json`    | 基于 TextMate grammar 的基础 token 高亮      |
| 语言配置   | `language-configuration.json`        | 配置注释、括号、自动闭合对                   |
| 代码片段   | `snippets/monkey.json`               | 提供常用 Monkey 语法片段                     |
| 解析诊断   | `src/extension.ts`                   | 保存/打开/编辑时调用 WASM parser             |
| 查看 AST   | `monkey.showAST`                     | 将当前文档解析为 JSON AST 并在新编辑器展示   |
| 编译字节码 | `monkey.compileToBytecode`           | 将当前文档编译为字节码文本并在新编辑器展示   |
| VSIX 打包  | `vsce package`                       | 基于 esbuild bundle 直接生成可安装包         |

### 2.2 暂不覆盖的能力

当前版本不实现以下能力：

- Language Server Protocol。
- 精准 range 诊断。
- hover、completion、rename、go to definition。
- 编辑器内运行 Monkey 程序。
- Marketplace 自动发布流程。

这些能力需要更稳定的语义模型、源码位置 span 和更完整的 WASM API 支撑。

---

## 3. 项目结构

```
packages/vscode-extension/
├── .vscodeignore                 # VSIX 打包排除规则
├── LICENSE                       # 随 VSIX 分发的许可证（与仓库根 LICENSE 一致）
├── README.md                     # 扩展使用说明
├── language-configuration.json   # VS Code 语言配置
├── package.json                  # VS Code extension manifest
├── scripts/
│   └── build.js                  # esbuild bundle + wasm 资产复制
├── snippets/
│   └── monkey.json               # Monkey 代码片段
├── src/
│   └── extension.ts              # 扩展运行时入口
├── syntaxes/
│   └── monkey.tmLanguage.json    # TextMate grammar
└── tsconfig.json                 # TypeScript 类型检查配置（noEmit）
```

生成物不进入 Git：

- `dist/`
- `node_modules/`
- `*.vsix`

其中 `dist/` 是 esbuild bundle 输出（`extension.js` 和 `monkey_wasm_bg.wasm`），`*.vsix` 是本地打包产物，`node_modules/` 由包管理器安装生成。

---

## 4. 扩展 Manifest 设计

VS Code extension 的主要声明在 `packages/vscode-extension/package.json` 中。

### 4.1 基础信息

```json
{
  "name": "monkey-extension",
  "displayName": "Monkey Language",
  "version": "0.12.1",
  "engines": {
    "vscode": "^1.74.0"
  },
  "main": "dist/extension.js"
}
```

关键点：

- `main` 指向编译后的 CommonJS 入口 `dist/extension.js`。
- `engines.vscode` 当前设为 `^1.74.0`，对应 `@types/vscode@1.74.0`。
- extension 版本和仓库根版本保持一致，由 release bump 脚本同步。

### 4.2 Activation Events

```json
[
  "onLanguage:monkey",
  "onCommand:monkey.compileToBytecode",
  "onCommand:monkey.showAST"
]
```

扩展只在需要时激活：

- 打开 Monkey 文件时激活，用于诊断。
- 执行 AST 或字节码命令时激活。

这样可以避免 VS Code 启动时加载扩展。

### 4.3 Contributions

扩展贡献项包括：

- `languages`：注册 `monkey` 语言和 `.monkey` 扩展名。
- `grammars`：注册 TextMate grammar。
- `snippets`：注册 Monkey snippet。
- `commands`：注册 AST 和字节码命令。
- `configuration`：注册 `monkey.enableWasmDiagnostics`。

`monkey.enableWasmDiagnostics` 默认为 `true`。关闭后扩展仍保留命令能力，但不会在打开或编辑文档时自动调用 parser。

---

## 5. 运行时架构

扩展运行时集中在 `src/extension.ts`。

```mermaid
flowchart TD
    A["VS Code activation"] --> B["createDiagnosticCollection"]
    B --> C{"enableWasmDiagnostics?"}
    C -->|yes| D["register document listeners"]
    C -->|no| E["skip diagnostics"]
    D --> F["validate Monkey documents"]
    F --> G["loadWasm()"]
    G --> H["@gengjiawen/monkey-wasm"]
    H --> I["parse / compile"]
    A --> J["register commands"]
    J --> K["Monkey: Show AST"]
    J --> L["Monkey: Compile To Bytecode"]
    K --> G
    L --> G
```

核心原则：

- 扩展激活时不立即加载 WASM。
- 第一次诊断或命令执行时才调用 `loadWasm()`。
- WASM module 通过 Promise 缓存，避免重复初始化。
- 所有用户可见错误通过 VS Code diagnostic 或 error message 展示。

---

## 6. WASM 加载策略

### 6.1 为什么不直接 `import('@gengjiawen/monkey-wasm')`

`@gengjiawen/monkey-wasm` 是 ESM 包，并且入口依赖 `.wasm` module import：

```js
import * as wasm from './monkey_wasm_bg.wasm'
```

这种加载方式依赖 Node/Electron 对 WASM ESM import 的支持，跨 VS Code 版本不够稳定。

### 6.2 当前实现

扩展源码通过 wasm-bindgen 生成的 package 入口复用 `parse` 和 `compile` 类型，同时从 bindings 子路径加载运行时代码；esbuild 会把 bindings 打进 CommonJS bundle，`.wasm` 字节则在运行时手动实例化：

1. `typeof import('@gengjiawen/monkey-wasm')` 复用 `wasm/pkg/monkey_wasm.d.ts` 中的公开 API 类型。
2. `require('@gengjiawen/monkey-wasm/monkey_wasm_bg.js')` 加载实际 bindings，构建时被 bundle 进 `dist/extension.js`。
3. 构建脚本把 `monkey_wasm_bg.wasm` 复制到 `dist/`，运行时通过 `join(__dirname, 'monkey_wasm_bg.wasm')` 定位。
4. 用 `readFileSync()` 读取 `.wasm` 字节。
5. 调用 `WebAssembly.instantiate(bytes, imports)`，imports 指向 bundle 内的 bindings。
6. 调用 bindings 的 `__wbg_set_wasm(instance.exports)`。
7. 如存在 `__wbindgen_start`，调用它完成初始化。

bindings 子路径没有自带类型声明，因此 extension 只在本地补充内部 `__wbg_set_wasm` 类型；`parse` 和 `compile` 类型来自生成包自带的 `monkey_wasm.d.ts`。

简化流程如下：

```mermaid
sequenceDiagram
    participant Extension as dist/extension.js（含 bindings）
    participant Wasm as dist/monkey_wasm_bg.wasm

    Extension->>Wasm: readFileSync()
    Extension->>Wasm: WebAssembly.instantiate(bytes, imports)
    Extension->>Extension: __wbg_set_wasm(instance.exports)
    Extension->>Wasm: __wbindgen_start()
    Extension->>Extension: parse() / compile()
```

### 6.3 依赖布局

运行时依赖使用 pnpm workspace 协议指向本地 `wasm/pkg`：

```json
{
  "dependencies": {
    "@gengjiawen/monkey-wasm": "workspace:*"
  }
}
```

选择 `workspace:*` 的原因：

- 本地开发、打包和 workspace 校验始终使用同一个仓库内的 WASM 生成包。
- VSIX 里的内容就是本地 `wasm/pkg` 的构建产物，不依赖 npm registry 上已发布的版本。
- extension 是仓库内的 private package，不需要维护 wasm 的 semver range。

由于扩展被完整 bundle，`dependencies` 只在构建期参与解析，VSIX 中不包含 `node_modules`。

---

## 7. 诊断与命令实现

### 7.1 诊断流程

诊断只对 `languageId === 'monkey'` 的文档运行。

触发时机：

- 文档打开：`onDidOpenTextDocument`
- 文档编辑：`onDidChangeTextDocument`
- 文档保存：`onDidSaveTextDocument`
- 扩展激活时已经打开的 Monkey 文档

当前诊断实现调用：

```typescript
mod.parse(text)
```

如果解析成功，清空文档 diagnostics。如果解析失败，把错误消息放到第一行第一列。

当前没有精准 range 的原因是 WASM parser 目前返回的是 AST JSON 字符串或抛出的错误文本，错误中没有稳定的源码 span。后续可以通过扩展 WASM API 支持结构化错误。

### 7.2 Show AST 命令

命令 id：

```text
monkey.showAST
```

行为：

1. 读取当前 active editor 文本。
2. 调用 `parse(text)`。
3. 打开一个临时 JSON 文档。
4. 将 AST JSON 作为内容展示。

### 7.3 Compile To Bytecode 命令

命令 id：

```text
monkey.compileToBytecode
```

行为：

1. 读取当前 active editor 文本。
2. 调用 `compile(text)`。
3. 打开一个临时 text 文档。
4. 将字节码文本作为内容展示。

---

## 8. 语言资源

### 8.1 TextMate Grammar

`syntaxes/monkey.tmLanguage.json` 提供基础语法高亮规则，覆盖：

- 注释。
- 字符串。
- 数字。
- 关键字。
- 内置标识符。
- 运算符。

TextMate grammar 只负责词法级高亮，不参与 parser 诊断，也不理解语义。

### 8.2 Language Configuration

`language-configuration.json` 配置：

- 行注释：`//`
- 块注释：`/* */`
- 括号对：`()`, `[]`, `{}`
- 自动闭合对。
- surrounding pairs。

这些配置由 VS Code 编辑器原生消费，用于括号匹配和自动补全。

### 8.3 Snippets

`snippets/monkey.json` 提供常用模板，例如：

- `let`
- `fn`
- `if`
- `ifelse`

snippet 只依赖 language id，不依赖扩展运行时代码。

---

## 9. 构建与打包

### 9.1 Build

推荐命令：

```bash
pnpm -C packages/vscode-extension run build
```

脚本定义：

```json
{
  "typecheck": "tsc -p .",
  "build": "pnpm run typecheck && node scripts/build.js",
  "watch": "node scripts/build.js --watch"
}
```

构建分为两步：

1. `tsc -p .` 只做类型检查（tsconfig 配置了 `noEmit`）。
2. `scripts/build.js` 用 esbuild 把 `src/extension.ts` bundle 成 `dist/extension.js`（CommonJS、`platform: node`、`external: ['vscode']`），并把 `monkey_wasm_bg.wasm` 从 workspace 依赖复制到 `dist/`。

`@gengjiawen/monkey-wasm` 的 bindings JS 被直接打进 bundle，运行时只剩 bundle 本身和旁边的 `.wasm` 资产。

### 9.2 Package

推荐命令：

```bash
pnpm -C packages/vscode-extension run package
```

`package` 就是普通的 `vsce package`：

- manifest 中配置了 `"vsce": { "dependencies": false }`（等价于 `--no-dependencies`），`vsce` 不再分析或打包 `node_modules`，因此 pnpm 的 symlink 布局和 `workspace:*` 协议都不会造成问题。这也是 oxc-vscode 等扩展使用的方案。
- `vsce` 会自动执行 `vscode:prepublish`（即 `pnpm run build`），保证打包内容是最新构建。
- `LICENSE` 直接提交在 `packages/vscode-extension/` 下（与仓库根 LICENSE 一致），随 VSIX 分发。

VSIX 中的 runtime 内容：

```text
extension/
└── dist/
    ├── extension.js          # 含 bindings 的完整 bundle
    └── monkey_wasm_bg.wasm
```

### 9.3 VSIX 排除规则

`.vscodeignore` 排除开发期文件：

```gitignore
src/**
scripts/**
node_modules/**
tsconfig.json
dist/**/*.map
*.vsix
```

`dist/` 不排除，因为它是 extension runtime 入口；sourcemap 只用于本地调试，不进入 VSIX。

---

## 10. 版本同步与发布维护

### 10.1 Extension 版本

`packages/vscode-extension/package.json#version` 与仓库根 `package.json#version` 保持一致。

版本同步由 `scripts/bump_cargo_packages.ts` 维护。release bump 时该脚本会同步：

- Rust workspace crate 版本。
- Playground 的 `@gengjiawen/monkey-wasm` workspace range。
- `prettier-plugin-monkey` 的 package 版本和 wasm 依赖。
- `vscode-extension` 的 package 版本和 wasm 依赖。
- `pnpm-lock.yaml`。

### 10.2 WASM 依赖版本

VS Code extension 在仓库内使用 workspace 依赖：

```json
{
  "@gengjiawen/monkey-wasm": "workspace:*"
}
```

由于扩展运行时代码完整 bundle 进 `dist/extension.js`，`workspace:*` 只在构建期解析到本地 `wasm/pkg`，打包阶段不需要任何版本转换或依赖安装。

### 10.3 pnpm v11 build approval

`pnpm-workspace.yaml` 中显式配置了允许执行 install/build 脚本的依赖：

```yaml
allowBuilds:
  '@swc/core': true
  '@vscode/vsce-sign': true
  esbuild: true
  keytar: true
```

这样可以避免 pnpm v11 在非交互环境下因为 build script approval 中断安装。

---

## 11. 测试与验证

### 11.1 常规验证命令

```bash
pnpm -C packages/vscode-extension run build
pnpm -C packages/vscode-extension run package
```

### 11.2 VSIX 内容检查

生成 VSIX 后可以检查关键 runtime 文件：

```bash
unzip -l packages/vscode-extension/monkey-extension-0.12.1.vsix \
  | rg 'extension/(dist/extension.js$|dist/monkey_wasm_bg.wasm$|package.json$|LICENSE.txt$)'
```

期望至少包含：

- `extension/dist/extension.js`
- `extension/dist/monkey_wasm_bg.wasm`
- `extension/package.json`
- `extension/LICENSE.txt`

不应包含 `node_modules/`、`src/`、`scripts/` 或 `*.map`。

### 11.3 手动 WASM smoke test

可以用 Node 验证 bundle 后的 wasm 加载路径是否能执行 parser/compiler（构建后运行）：

```bash
node - <<'NODE'
const { readFileSync } = require('fs')

async function main() {
  const distDir = 'packages/vscode-extension/dist'
  const bindings = require.resolve(
    '@gengjiawen/monkey-wasm/monkey_wasm_bg.js',
    { paths: ['packages/vscode-extension'] }
  )
  const mod = await import(require('url').pathToFileURL(bindings).href)
  const { instance } = await WebAssembly.instantiate(
    readFileSync(`${distDir}/monkey_wasm_bg.wasm`),
    { './monkey_wasm_bg.js': mod }
  )
  mod.__wbg_set_wasm(instance.exports)
  instance.exports.__wbindgen_start()
  console.log(mod.parse('let x = 1;').includes('Program'))
  console.log(mod.compile('let x = 1;').length > 0)
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
NODE
```

### 11.4 仓库级验证

扩展改动通常还应跑：

```bash
./node_modules/.bin/prettier --check \
  docs/vscode-extension-design.md \
  packages/vscode-extension/package.json \
  packages/vscode-extension/scripts/build.js \
  packages/vscode-extension/src/extension.ts

git diff --check
cargo test
```

---

## 12. 后续演进

### 12.1 结构化诊断

当前诊断只能把错误放到第一行第一列。更好的方案是在 Rust parser 或 WASM wrapper 中返回结构化错误：

```typescript
type MonkeyDiagnostic = {
  message: string
  startLine: number
  startColumn: number
  endLine: number
  endColumn: number
}
```

这样 VS Code 可以标记精确源码范围。

### 12.2 Language Server

当需要 hover、completion、definition、rename 等语义能力时，可以引入 LSP：

```mermaid
flowchart LR
    A["VS Code Extension"] --> B["Language Client"]
    B --> C["Monkey Language Server"]
    C --> D["Parser / Compiler"]
```

LSP 会增加架构复杂度，但可以把编辑器协议和语言能力解耦，适合更完整的 IDE 能力。

### 12.3 Marketplace 发布

当前扩展可以打包为 VSIX。若要发布到 Marketplace，还需要补齐：

- 实际 publisher。
- icon、gallery banner 等展示信息。
- changelog。
- CI 中的 `vsce publish` 或 `ovsx publish` 流程。
- 发布 token 管理。

### 12.4 WASM 包形态优化

后续可以考虑让 `@gengjiawen/monkey-wasm` 同时发布适合 Node/CommonJS extension host 的入口，例如：

- 显式 Node target。
- CommonJS wrapper。
- 非实验性的 `.wasm` 加载 API。

这样 VS Code extension 可以减少当前手动初始化 wasm-bindgen bindings 的代码。
