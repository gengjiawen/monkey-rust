import { execSync } from "child_process"
import { readFileSync, writeFileSync } from "fs"
import { join } from "path"

const pkgPath = join(__dirname, "..", "package.json")
const pak = JSON.parse(readFileSync(pkgPath, "utf-8"))

const bump_cmd = `cargo workspaces version custom ${pak.version} --no-git-commit -y`
console.log(bump_cmd)
execSync(bump_cmd)

// Also bump playground dependency on @gengjiawen/monkey-wasm
try {
  const playgroundPkgPath = join(
    __dirname,
    "..",
    "packages",
    "playground",
    "package.json",
  )
  const playgroundRaw = readFileSync(playgroundPkgPath, "utf-8")
  const playground = JSON.parse(playgroundRaw)
  if (
    playground.dependencies &&
    playground.dependencies["@gengjiawen/monkey-wasm"]
  ) {
    const newRange = `workspace:^${pak.version}`
    const prev = playground.dependencies["@gengjiawen/monkey-wasm"]
    if (prev !== newRange) {
      playground.dependencies["@gengjiawen/monkey-wasm"] = newRange
      writeFileSync(
        playgroundPkgPath,
        JSON.stringify(playground, null, 2) + "\n",
        "utf-8",
      )
      console.log(
        `Updated playground dependency @gengjiawen/monkey-wasm: ${prev} -> ${newRange}`,
      )
    } else {
      console.log(
        `Playground dependency already up-to-date: ${prev}`,
      )
    }
  } else {
    console.log(
      "Playground package.json missing @gengjiawen/monkey-wasm dependency; skipped.",
    )
  }
} catch (e) {
  console.warn("Failed to update playground dependency:", e)
}

// Also keep prettier-plugin-monkey package version and wasm dependency in sync
try {
  const prettierPluginPkgPath = join(
    __dirname,
    "..",
    "packages",
    "prettier-plugin-monkey",
    "package.json",
  )
  const prettierPluginRaw = readFileSync(prettierPluginPkgPath, "utf-8")
  const prettierPlugin = JSON.parse(prettierPluginRaw)
  let prettierPluginChanged = false

  if (prettierPlugin.version !== pak.version) {
    const prevVersion = prettierPlugin.version
    prettierPlugin.version = pak.version
    prettierPluginChanged = true
    console.log(
      `Updated prettier-plugin-monkey version: ${prevVersion} -> ${pak.version}`,
    )
  } else {
    console.log(
      `prettier-plugin-monkey version already up-to-date: ${prettierPlugin.version}`,
    )
  }

  if (
    prettierPlugin.dependencies &&
    prettierPlugin.dependencies["@gengjiawen/monkey-wasm"]
  ) {
    const newRange = `^${pak.version}`
    const prev = prettierPlugin.dependencies["@gengjiawen/monkey-wasm"]
    if (prev !== newRange) {
      prettierPlugin.dependencies["@gengjiawen/monkey-wasm"] = newRange
      prettierPluginChanged = true
      console.log(
        `Updated prettier-plugin-monkey dependency @gengjiawen/monkey-wasm: ${prev} -> ${newRange}`,
      )
    } else {
      console.log(
        `prettier-plugin-monkey dependency already up-to-date: ${prev}`,
      )
    }
  } else {
    console.log(
      "prettier-plugin-monkey package.json missing @gengjiawen/monkey-wasm dependency; skipped.",
    )
  }

  if (prettierPluginChanged) {
    writeFileSync(
      prettierPluginPkgPath,
      JSON.stringify(prettierPlugin, null, 2) + "\n",
      "utf-8",
    )
  }
} catch (e) {
  console.warn("Failed to update prettier-plugin-monkey dependency:", e)
}
