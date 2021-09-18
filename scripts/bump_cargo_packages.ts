import { execSync } from "child_process"

const pak = require('../package.json')
console.log(pak)

execSync(`cargo install cargo-workspaces`)
const bump_cmd = `cargo workspaces version custom ${pak.version} --no-git-commit -y`
console.log(bump_cmd)
execSync(bump_cmd)