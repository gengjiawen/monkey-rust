import { execSync } from "child_process"
import { readFileSync } from "fs"
import { join } from "path"

const pkgPath = join(__dirname, "..", "package.json")
const pak = JSON.parse(readFileSync(pkgPath, "utf-8"))
console.log(pak)

const bump_cmd = `cargo workspaces version custom ${pak.version} --no-git-commit -y`
console.log(bump_cmd)
execSync(bump_cmd)
