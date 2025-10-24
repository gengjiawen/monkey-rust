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
