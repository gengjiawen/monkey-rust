const { execFileSync } = require('child_process')
const {
  copyFileSync,
  cpSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} = require('fs')
const { tmpdir } = require('os')
const { basename, join, resolve } = require('path')

const extensionRoot = resolve(__dirname, '..')
const repoRoot = resolve(extensionRoot, '..', '..')
const packageJsonPath = join(extensionRoot, 'package.json')
const packageJson = JSON.parse(readFileSync(packageJsonPath, 'utf-8'))
const stagingDir = join(tmpdir(), `monkey-extension-package-${process.pid}`)
const vsixName = `${packageJson.name}-${packageJson.version}.vsix`
const tscBin = process.platform === 'win32' ? 'tsc.cmd' : 'tsc'
const vsceBin = process.platform === 'win32' ? 'vsce.cmd' : 'vsce'

function copyEntry(entry) {
  cpSync(join(extensionRoot, entry), join(stagingDir, entry), {
    recursive: true,
  })
}

rmSync(stagingDir, { recursive: true, force: true })
mkdirSync(stagingDir, { recursive: true })

execFileSync(tscBin, ['-p', '.'], {
  cwd: extensionRoot,
  stdio: 'inherit',
})

for (const entry of [
  '.vscodeignore',
  'README.md',
  'dist',
  'language-configuration.json',
  'snippets',
  'syntaxes',
]) {
  copyEntry(entry)
}

copyFileSync(join(repoRoot, 'LICENSE'), join(stagingDir, 'LICENSE'))

const packagedManifest = {
  ...packageJson,
}
delete packagedManifest.devDependencies
delete packagedManifest.scripts

writeFileSync(
  join(stagingDir, 'package.json'),
  JSON.stringify(packagedManifest, null, 2) + '\n',
  'utf-8'
)

execFileSync(
  'npm',
  ['install', '--omit=dev', '--package-lock=false', '--ignore-scripts'],
  {
    cwd: stagingDir,
    stdio: 'inherit',
  }
)

execFileSync(vsceBin, ['package'], {
  cwd: stagingDir,
  stdio: 'inherit',
})

copyFileSync(
  join(stagingDir, vsixName),
  join(extensionRoot, basename(vsixName))
)
rmSync(stagingDir, { recursive: true, force: true })
