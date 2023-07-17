import { execSync } from "child_process"

const pak = require('../package.json')
console.log(pak)

execSync(`cargo install cargo-workspaces`)
console.log(`install rust deps done`)
const bump_cmd = `cargo workspaces version custom ${pak.version} --no-git-commit -y`
console.log(bump_cmd)
execSync(bump_cmd)