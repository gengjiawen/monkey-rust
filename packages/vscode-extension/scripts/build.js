const { copyFileSync, mkdirSync } = require('fs')
const { join, resolve } = require('path')
const esbuild = require('esbuild')

const extensionRoot = resolve(__dirname, '..')
const distDir = join(extensionRoot, 'dist')
const watch = process.argv.includes('--watch')

const buildOptions = {
  entryPoints: [join(extensionRoot, 'src', 'extension.ts')],
  outfile: join(distDir, 'extension.js'),
  bundle: true,
  platform: 'node',
  format: 'cjs',
  target: 'node16',
  external: ['vscode'],
  sourcemap: true,
  logLevel: 'info',
}

function copyWasmAssets() {
  mkdirSync(distDir, { recursive: true })
  const wasmPath = require.resolve(
    '@gengjiawen/monkey-wasm/monkey_wasm_bg.wasm',
    {
      paths: [extensionRoot],
    }
  )
  copyFileSync(wasmPath, join(distDir, 'monkey_wasm_bg.wasm'))
}

async function main() {
  copyWasmAssets()
  if (watch) {
    const ctx = await esbuild.context(buildOptions)
    await ctx.watch()
  } else {
    await esbuild.build(buildOptions)
  }
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
