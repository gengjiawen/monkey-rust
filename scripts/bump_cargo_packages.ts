import { execSync } from 'child_process'
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'fs'
import { join } from 'path'

type PackageJson = {
  version?: string
  dependencies?: Record<string, string>
  [key: string]: unknown
}

const rootPath = join(__dirname, '..')

function repoPath(...parts: string[]) {
  return join(rootPath, ...parts)
}

function readPackageJson(path: string): PackageJson {
  return JSON.parse(readFileSync(path, 'utf-8')) as PackageJson
}

function writePackageJson(path: string, packageJson: PackageJson) {
  writeFileSync(path, JSON.stringify(packageJson, null, 2) + '\n', 'utf-8')
}

function syncPackageVersion(
  packageJson: PackageJson,
  packageName: string,
  nextVersion: string
) {
  if (packageJson.version !== nextVersion) {
    const prevVersion = packageJson.version
    packageJson.version = nextVersion
    console.log(
      `Updated ${packageName} version: ${prevVersion} -> ${nextVersion}`
    )
    return true
  }

  console.log(
    `${packageName} version already up-to-date: ${packageJson.version}`
  )
  return false
}

function syncDependencyRange(
  packageJson: PackageJson,
  packageName: string,
  dependencyName: string,
  nextRange: string
) {
  if (!packageJson.dependencies || !packageJson.dependencies[dependencyName]) {
    console.log(
      `${packageName} package.json missing ${dependencyName} dependency; skipped.`
    )
    return false
  }

  const prev = packageJson.dependencies[dependencyName]
  if (prev !== nextRange) {
    packageJson.dependencies[dependencyName] = nextRange
    console.log(
      `Updated ${packageName} dependency ${dependencyName}: ${prev} -> ${nextRange}`
    )
    return true
  }

  console.log(`${packageName} dependency already up-to-date: ${prev}`)
  return false
}

const pkgPath = repoPath('package.json')
const rootPackage = readPackageJson(pkgPath)
if (
  typeof rootPackage.version !== 'string' ||
  rootPackage.version.length === 0
) {
  throw new Error('Root package.json is missing a valid version')
}

const nextVersion = rootPackage.version
const bump_cmd = `cargo workspaces version custom ${nextVersion} --no-git-commit -y`
console.log(bump_cmd)
execSync(bump_cmd)

// Also bump playground dependencies on the versioned Monkey packages.
try {
  const playgroundPkgPath = repoPath('packages', 'playground', 'package.json')
  const playground = readPackageJson(playgroundPkgPath)
  let playgroundChanged = false

  playgroundChanged =
    syncDependencyRange(
      playground,
      'playground',
      '@gengjiawen/monkey-wasm',
      `workspace:^${nextVersion}`
    ) || playgroundChanged
  playgroundChanged =
    syncDependencyRange(
      playground,
      'playground',
      '@gengjiawen/monkey-minifier',
      `workspace:^${nextVersion}`
    ) || playgroundChanged

  if (playgroundChanged) {
    writePackageJson(playgroundPkgPath, playground)
  }
} catch (e) {
  console.warn('Failed to update playground dependency:', e)
}

// Also keep prettier-plugin-monkey package version and wasm dependency in sync
try {
  const prettierPluginPkgPath = repoPath(
    'packages',
    'prettier-plugin-monkey',
    'package.json'
  )
  const prettierPlugin = readPackageJson(prettierPluginPkgPath)
  let prettierPluginChanged = false

  prettierPluginChanged =
    syncPackageVersion(prettierPlugin, 'prettier-plugin-monkey', nextVersion) ||
    prettierPluginChanged
  prettierPluginChanged =
    syncDependencyRange(
      prettierPlugin,
      'prettier-plugin-monkey',
      '@gengjiawen/monkey-wasm',
      `^${nextVersion}`
    ) || prettierPluginChanged

  if (prettierPluginChanged) {
    writePackageJson(prettierPluginPkgPath, prettierPlugin)
  }
} catch (e) {
  console.warn('Failed to update prettier-plugin-monkey dependency:', e)
}

// Also keep monkey-minifier package version and wasm dependency in sync.
// The repository uses workspace: during development, while release PRs must
// contain a registry-compatible range because npm publish preserves
// workspace: specifiers verbatim.
try {
  const minifierPkgPath = repoPath(
    'packages',
    'monkey-minifier',
    'package.json'
  )
  const minifier = readPackageJson(minifierPkgPath)
  let minifierChanged = false

  minifierChanged =
    syncPackageVersion(minifier, 'monkey-minifier', nextVersion) ||
    minifierChanged
  minifierChanged =
    syncDependencyRange(
      minifier,
      'monkey-minifier',
      '@gengjiawen/monkey-wasm',
      `^${nextVersion}`
    ) || minifierChanged

  if (minifierChanged) {
    writePackageJson(minifierPkgPath, minifier)
  }
} catch (e) {
  console.warn('Failed to update monkey-minifier dependency:', e)
}

// Also keep vscode-extension package version in sync
try {
  const vscodeExtensionPkgPath = repoPath(
    'packages',
    'vscode-extension',
    'package.json'
  )
  const vscodeExtension = readPackageJson(vscodeExtensionPkgPath)
  let vscodeExtensionChanged = false

  vscodeExtensionChanged =
    syncPackageVersion(vscodeExtension, 'vscode-extension', nextVersion) ||
    vscodeExtensionChanged
  vscodeExtensionChanged =
    syncDependencyRange(
      vscodeExtension,
      'vscode-extension',
      '@gengjiawen/monkey-wasm',
      'workspace:*'
    ) || vscodeExtensionChanged

  if (vscodeExtensionChanged) {
    writePackageJson(vscodeExtensionPkgPath, vscodeExtension)
  }
} catch (e) {
  console.warn('Failed to update vscode-extension version:', e)
}

// Keep pnpm-lock.yaml in sync with release-please package.json updates.
// wasm/pkg is generated later by wasm-pack, so create a minimal manifest
// temporarily when refreshing the lockfile on a release PR branch.
const wasmPkgDir = repoPath('wasm', 'pkg')
const wasmPkgPath = repoPath('wasm', 'pkg', 'package.json')
const hadWasmPkgDir = existsSync(wasmPkgDir)
const hadWasmPkgPackage = existsSync(wasmPkgPath)
const originalWasmPkgPackage = hadWasmPkgPackage
  ? readFileSync(wasmPkgPath, 'utf-8')
  : undefined

try {
  mkdirSync(wasmPkgDir, { recursive: true })
  writeFileSync(
    wasmPkgPath,
    JSON.stringify(
      {
        name: '@gengjiawen/monkey-wasm',
        version: nextVersion,
      },
      null,
      2
    ) + '\n',
    'utf-8'
  )

  execSync('pnpm install --lockfile-only --link-workspace-packages=true', {
    cwd: rootPath,
    stdio: 'inherit',
  })
} finally {
  if (hadWasmPkgPackage && originalWasmPkgPackage !== undefined) {
    writeFileSync(wasmPkgPath, originalWasmPkgPackage, 'utf-8')
  } else {
    rmSync(wasmPkgPath, { force: true })
  }
  if (!hadWasmPkgDir) {
    rmSync(wasmPkgDir, { recursive: true, force: true })
  }
}
