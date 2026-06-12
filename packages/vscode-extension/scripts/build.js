const { execFileSync } = require('child_process')
const { existsSync } = require('fs')
const { join, resolve } = require('path')

const extensionRoot = resolve(__dirname, '..')
const repoRoot = resolve(extensionRoot, '..', '..')
const binDir = join(repoRoot, 'node_modules', '.bin')
const tscBin = join(binDir, process.platform === 'win32' ? 'tsc.cmd' : 'tsc')

if (!existsSync(tscBin)) {
  throw new Error(
    'Missing TypeScript binary. Run pnpm install at the repo root.'
  )
}

execFileSync(tscBin, ['-p', '.'], {
  cwd: extensionRoot,
  stdio: 'inherit',
})
