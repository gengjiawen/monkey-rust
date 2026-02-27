# Changelog

## [0.12.0](https://github.com/gengjiawen/monkey-rust/compare/v0.11.1...v0.12.0) (2026-02-27)


### Features

* **playground:** integrate prettier-plugin-monkey for code formatting ([bf52a0a](https://github.com/gengjiawen/monkey-rust/commit/bf52a0a1e6ad4e32a40d46144ecf0ce88787b68f))


### Bug Fixes

* **release:** sync prettier plugin version and refresh workflow ([3608f04](https://github.com/gengjiawen/monkey-rust/commit/3608f0471e62feaec5d322fa93bfdf0d4464ba39))

### [0.11.1](https://www.github.com/gengjiawen/monkey-rust/compare/v0.11.0...v0.11.1) (2026-02-24)


### Bug Fixes

* **ci:** use setup-node v4 for lts/* in release workflow ([c17b1d5](https://www.github.com/gengjiawen/monkey-rust/commit/c17b1d580f9c628d71b67035675149d18ec96acd))
* **playground:** use workspace:* for monkey-wasm dependency ([eb393a0](https://www.github.com/gengjiawen/monkey-rust/commit/eb393a07d3505352fbd738a1be03b9109865ffe9))

## [0.11.0](https://www.github.com/gengjiawen/monkey-rust/compare/v0.10.1...v0.11.0) (2026-02-24)


### Features

* **prettier-plugin:** add monkey prettier plugin and development guide ([e294acd](https://www.github.com/gengjiawen/monkey-rust/commit/e294acd6ca1939450999ef0897ec25be7867d01e))


### Bug Fixes

* playground wasm version ([e3fbb64](https://www.github.com/gengjiawen/monkey-rust/commit/e3fbb64f96581553d05a44c422c04faa2746fbba))

### [0.10.1](https://www.github.com/gengjiawen/monkey-rust/compare/v0.10.0...v0.10.1) (2025-10-24)


### Bug Fixes

* ci publish ([868506c](https://www.github.com/gengjiawen/monkey-rust/commit/868506c0819b885f7b90b558db88ba4ed4facff1))
* version bump ([639585d](https://www.github.com/gengjiawen/monkey-rust/commit/639585dce22b89aba58188248ef4830589a05769))

## [0.10.0](https://www.github.com/gengjiawen/monkey-rust/compare/v0.9.2...v0.10.0) (2025-10-24)


### Features

* fix compatibility with latest rust ([63bc3f1](https://www.github.com/gengjiawen/monkey-rust/commit/63bc3f1016a3338e145d0af7deb26c1325f0530a))


### Bug Fixes

* compiler symbol resolve bug ([4f0c3eb](https://www.github.com/gengjiawen/monkey-rust/commit/4f0c3ebac18f4ecc5a8f6594475dc92db5efa839))
* lexer parse comment bug ([51014cc](https://www.github.com/gengjiawen/monkey-rust/commit/51014ccb7282a6ca23622cf606cc4026ab8ab3cf))
* remove deprecated packages ([0f04fe6](https://www.github.com/gengjiawen/monkey-rust/commit/0f04fe61bed552c0700cc0efe367d98abf1fdebb))

### [0.9.2](https://www.github.com/gengjiawen/monkey-rust/compare/v0.9.1...v0.9.2) (2024-08-12)


### Bug Fixes

* bump base docker ([393f3c4](https://www.github.com/gengjiawen/monkey-rust/commit/393f3c4d3af47dacf8d7f399153435daec9222f7))
* remove redux ([66ff65b](https://www.github.com/gengjiawen/monkey-rust/commit/66ff65b60df664d65982cb3551f18889b62e3b09))


### Dev

* bump dev env ([bc54a13](https://www.github.com/gengjiawen/monkey-rust/commit/bc54a13c985c5bcdf2a4b72a58e22717d0682bd8))

### [0.9.1](https://www.github.com/gengjiawen/monkey-rust/compare/v0.9.0...v0.9.1) (2023-07-17)


### Bug Fixes

* prepare rust tag issue ([4b4932e](https://www.github.com/gengjiawen/monkey-rust/commit/4b4932ea880ce650d95b60e6eb73977e637cde09))
* script update ([873e2ad](https://www.github.com/gengjiawen/monkey-rust/commit/873e2adfa265564b6fe0760f1d293c3508b354c8))
* Update prepare-release.yml ([f1e85c9](https://www.github.com/gengjiawen/monkey-rust/commit/f1e85c9ace989d35faf3faa04be34f2508cc0864))

## [0.9.0](https://www.github.com/gengjiawen/monkey-rust/compare/v0.8.0...v0.9.0) (2023-07-17)


### Features

* add auto deploy ([e8b03de](https://www.github.com/gengjiawen/monkey-rust/commit/e8b03ded807240d55777cf25211f9aca620ae668))
* add binary calculate for compiler ([22f53d0](https://www.github.com/gengjiawen/monkey-rust/commit/22f53d0691a15f300d4172fcfddd287b81e442e7))
* add bundle visualizer ([11d2dc8](https://www.github.com/gengjiawen/monkey-rust/commit/11d2dc82a7f880e3601b4a49c10433f32bd47ccd))
* add compiler to wasm ([7cd371a](https://www.github.com/gengjiawen/monkey-rust/commit/7cd371aa1f507210726abb3fc29780e3b355708f))
* add console hook for wasm ([34dc010](https://www.github.com/gengjiawen/monkey-rust/commit/34dc0105eace8da751a0a529c7014560b1afdd92))
* add equal notEqual greatThan lessThan support ([9b8fee6](https://www.github.com/gengjiawen/monkey-rust/commit/9b8fee68df6798f143c81ac4138e45c6300afa80))
* add error handing ([bf4219f](https://www.github.com/gengjiawen/monkey-rust/commit/bf4219fc38ea8099e817c5be3551554555c66f22))
* add func name in parser ([ca64af0](https://www.github.com/gengjiawen/monkey-rust/commit/ca64af0e8bb9e5666f7acd05e0c7002190e094d9))
* add if node alternate support ([56ff64e](https://www.github.com/gengjiawen/monkey-rust/commit/56ff64e5cbe850381b40aa6988003f6706726872))
* add more compiler test ([c6c5df3](https://www.github.com/gengjiawen/monkey-rust/commit/c6c5df3ee228703ec727cab055853edf71f61913))
* add more vm test ([aed972d](https://www.github.com/gengjiawen/monkey-rust/commit/aed972deb2b38963aa2cb2946c036a3a6a8dd6fb))
* add null ([6c19dd0](https://www.github.com/gengjiawen/monkey-rust/commit/6c19dd079453183a1a29970febd6d9a2a86c4a47))
* add prefix support ([016ae79](https://www.github.com/gengjiawen/monkey-rust/commit/016ae7901866f135cb4ce382b71db5b2938ceed3))
* expose more opcode method ([513a15d](https://www.github.com/gengjiawen/monkey-rust/commit/513a15d28a5b693d17acc3aeae5eebd37f30bcde))
* finish builtins ([843ce30](https://www.github.com/gengjiawen/monkey-rust/commit/843ce3086dea1c5b23020c5aea59a15d29fadd71))
* finish condition execute ([ad3860e](https://www.github.com/gengjiawen/monkey-rust/commit/ad3860e1d39559ea217b29da9c6cc6a310ab0620))
* finish condition feature ([3b223ff](https://www.github.com/gengjiawen/monkey-rust/commit/3b223ff74485cbac6eb55a05cf1aa529253e69fe))
* finish name bindings ([43df97e](https://www.github.com/gengjiawen/monkey-rust/commit/43df97e211d4bf78648c6824b7560be88b0b172f))
* implement boolean ([32e3fdb](https://www.github.com/gengjiawen/monkey-rust/commit/32e3fdb6a6bf1bdadf54b6e539a9d4e276fbbb9a))
* implement eq for object ([1c1c3e2](https://www.github.com/gengjiawen/monkey-rust/commit/1c1c3e25b890793f002d357e1359d7c1be93a117))
* initial compiler ([#78](https://www.github.com/gengjiawen/monkey-rust/issues/78)) ([7866b8f](https://www.github.com/gengjiawen/monkey-rust/commit/7866b8fab120adc391a51c36cdd5a7f153400f14))
* initial function impl ([723de32](https://www.github.com/gengjiawen/monkey-rust/commit/723de324ce0dec1e148a56490e36b21fe491d8a9))
* initial online playground ([ec7732f](https://www.github.com/gengjiawen/monkey-rust/commit/ec7732f2fec8cc31abb1e7bf1c67234cc7605c6f))
* reformat use rules ([1f7c47a](https://www.github.com/gengjiawen/monkey-rust/commit/1f7c47af9e39f9de8e68f6471670e17b33a97441))
* split vm test ([001405c](https://www.github.com/gengjiawen/monkey-rust/commit/001405cbb1bc905313992f878fc86589db69bcab))
* start use cargo fmt ([e52c502](https://www.github.com/gengjiawen/monkey-rust/commit/e52c502334842535842b179c3503e039e78e6499))
* support closure ([#147](https://www.github.com/gengjiawen/monkey-rust/issues/147)) ([397284c](https://www.github.com/gengjiawen/monkey-rust/commit/397284ce3dc84a09fde4094e8517ac1358a24930))
* update ast-site ([949b9ad](https://www.github.com/gengjiawen/monkey-rust/commit/949b9ad59885ded047010cce92a9fbd658cce8f6))
* use new sample ([b88bbd1](https://www.github.com/gengjiawen/monkey-rust/commit/b88bbd18ae9e9e78fb7a20f211804e361979d5cf))
* use wasm back ([e1db4ce](https://www.github.com/gengjiawen/monkey-rust/commit/e1db4ce47f539fb5edf6341da91b5701538f7491))


### Bug Fixes

* add initial wasm test ([c9fe06e](https://www.github.com/gengjiawen/monkey-rust/commit/c9fe06e78139365a6af7a9e19a6cc44ed8412871))
* add more concrete test for opcode ([ad7fe4d](https://www.github.com/gengjiawen/monkey-rust/commit/ad7fe4d5868cb557d3910aea9cb214ef938d7232))
* add test for symbol tables ([0effa16](https://www.github.com/gengjiawen/monkey-rust/commit/0effa1663613b89c24919089028a35bc0882f59f))
* circle CI ([befb04e](https://www.github.com/gengjiawen/monkey-rust/commit/befb04e3bfccd33bd64a6209f336d96021fa12ac))
* circleCI ([8683cc3](https://www.github.com/gengjiawen/monkey-rust/commit/8683cc378a437b1e859e787f13c035a391438277))
* compiler repl ([#109](https://www.github.com/gengjiawen/monkey-rust/issues/109)) ([34b23a8](https://www.github.com/gengjiawen/monkey-rust/commit/34b23a8676de6d0e268cc2a91ca581f8da9306f1))
* compiler warnings ([0af35a8](https://www.github.com/gengjiawen/monkey-rust/commit/0af35a8ab17125eb6ed0915d7f0de7d2f654eff2))
* compiler warnings ([4d858f6](https://www.github.com/gengjiawen/monkey-rust/commit/4d858f65fbfac5a1ea7f4603cf2a377b3de4826d))
* dev docker name ([35485ce](https://www.github.com/gengjiawen/monkey-rust/commit/35485ce9c5f596cd1982a1be53f84a2edbefdd2b))
* docker setup ([bb4f698](https://www.github.com/gengjiawen/monkey-rust/commit/bb4f6988e14a6fd663db42965bf2cd54bdd7fa5f))
* fn test ([589b322](https://www.github.com/gengjiawen/monkey-rust/commit/589b322e298f69b212862cd100322c83f10f6c40))
* gh deploy with vite ([fcd7a38](https://www.github.com/gengjiawen/monkey-rust/commit/fcd7a382c0a3edcf89a3102dd3404242521ca067))
* github action patch ([#103](https://www.github.com/gengjiawen/monkey-rust/issues/103)) ([2c35ac3](https://www.github.com/gengjiawen/monkey-rust/commit/2c35ac3e3b5c7bdb3b1506d88eddf5c405af8d22))
* limit crates keywords ([8260313](https://www.github.com/gengjiawen/monkey-rust/commit/82603133eead3149165638cdcbf8ac6a64852f64))
* opArray description ([64971df](https://www.github.com/gengjiawen/monkey-rust/commit/64971dfeacbac06033577dbacd8aad5496ed5a80))
* opcode description ([9ef1450](https://www.github.com/gengjiawen/monkey-rust/commit/9ef1450140909eeed4f5804e9e289a7965d86b9e))
* pnpm 8 issues ([7ef925b](https://www.github.com/gengjiawen/monkey-rust/commit/7ef925b5338f1997ccab7468735ab845229804a7))
* prod build ([5ddbc95](https://www.github.com/gengjiawen/monkey-rust/commit/5ddbc95156a91def6a27f7459142968627acf982))
* refactor hash elements ([d5c7d3d](https://www.github.com/gengjiawen/monkey-rust/commit/d5c7d3d6a3bcaad98ba2f5979a4a1da3834789b5))
* revert to old way ([a44a2d6](https://www.github.com/gengjiawen/monkey-rust/commit/a44a2d6fce59807ee7a33d765bd0f63dcfcfb4a1))
* test wasm on headless browsers ([0eb9e07](https://www.github.com/gengjiawen/monkey-rust/commit/0eb9e07bf9841c5cb5f7eaef81698f05f3bdd7ad))
* try full docker image ([263875c](https://www.github.com/gengjiawen/monkey-rust/commit/263875c245596a3e0a279b6da69dcf488569fff4))
* use new rust analyser ([6bd438e](https://www.github.com/gengjiawen/monkey-rust/commit/6bd438eac4149819a2dcdb06daca6ecf20036b19))
* vite wasm ([3fa01b6](https://www.github.com/gengjiawen/monkey-rust/commit/3fa01b6a24c7707631f9180947e71314aab7557f))
* wrong compiler warning ([2f9c32d](https://www.github.com/gengjiawen/monkey-rust/commit/2f9c32ddb7ad0796eff348146fb1b411d9dfa362))


### playground

* add sample list ([c958cf3](https://www.github.com/gengjiawen/monkey-rust/commit/c958cf3c6a92b5c0dd1e0cfb1a95c7d170fa958e))


### Doc

* add favicon ([f4bd89e](https://www.github.com/gengjiawen/monkey-rust/commit/f4bd89e32a81c02a2f1c45ce2d79585d71241c9e))
* only publish main ([26a619b](https://www.github.com/gengjiawen/monkey-rust/commit/26a619bac00cb59b155ff4039bd888c692a32287))
* split maintainer job ([70c62e0](https://www.github.com/gengjiawen/monkey-rust/commit/70c62e030dd7bbe26fac6868409e71a339803112))
* update README ([76f6472](https://www.github.com/gengjiawen/monkey-rust/commit/76f6472aa7282e33abdd13ac9e70729bb1df1ecb))
* update README ([4691b46](https://www.github.com/gengjiawen/monkey-rust/commit/4691b46e0fe143dfa954b6a7ff8734701b0c7b3d))
* Update README ([d423e0c](https://www.github.com/gengjiawen/monkey-rust/commit/d423e0c99641b814ea7d303c83daab3ec86f7112))


### compiler

* finish array ([e1c1ee5](https://www.github.com/gengjiawen/monkey-rust/commit/e1c1ee5dbdc5e71b2c0019cffb884d36c8096d91))
* finish hashmap ([dc59660](https://www.github.com/gengjiawen/monkey-rust/commit/dc59660ba66c3e7a93c3e3ca9d9b0df133a90e9f))
* finish index ([2940eab](https://www.github.com/gengjiawen/monkey-rust/commit/2940eab1b9e0bf2989b767c8dfa73f6095c1356b))
* fix repl ([5c51ede](https://www.github.com/gengjiawen/monkey-rust/commit/5c51ede0b83dda5b1feea4c05422978ac4fd30ff))
* implement string ([ac53c64](https://www.github.com/gengjiawen/monkey-rust/commit/ac53c6464eb30c2df67f0839d194ba57b5cb8e92))


### Dev

* add ast website to workspaces ([372701c](https://www.github.com/gengjiawen/monkey-rust/commit/372701cbdc6ac80ca16da10c6b27a138ba5a92fa))
* add compiler to dev log ([bc36cc5](https://www.github.com/gengjiawen/monkey-rust/commit/bc36cc5f70461eb63bbc71e41193e26594576445))
* add missing toolchain ([75a505c](https://www.github.com/gengjiawen/monkey-rust/commit/75a505c62e31037de96fd71042e840bdcdf1d24a))
* add prettier ([e08322b](https://www.github.com/gengjiawen/monkey-rust/commit/e08322b7cca388f3a08e3e291cded6fe09e6cc66))
* bump base image ([75cc281](https://www.github.com/gengjiawen/monkey-rust/commit/75cc28132801277a609aad9d6738c724d24305f0))
* bump env ([ba4473f](https://www.github.com/gengjiawen/monkey-rust/commit/ba4473f89c956bc271f75f5349e48b9ca717e5ba))
* bump wasi-sdk ([#97](https://www.github.com/gengjiawen/monkey-rust/issues/97)) ([507ea65](https://www.github.com/gengjiawen/monkey-rust/commit/507ea656f9bc16f0d4b5fbe2145382e0a48fe2ab))
* switch to pnpm ([ce34c05](https://www.github.com/gengjiawen/monkey-rust/commit/ce34c05fee2408a6f2b9ebabd10f45f73d33ce28))
* trigger gitpod rebuild ([5113204](https://www.github.com/gengjiawen/monkey-rust/commit/5113204efb882196ec4720bfa88b83b7e27d6df7))
* update base docker ([ad0b292](https://www.github.com/gengjiawen/monkey-rust/commit/ad0b292edb03ade0893bfa9d0c2d55d7c210b3ee))
* update base env ([ecec748](https://www.github.com/gengjiawen/monkey-rust/commit/ecec748201348754ad4f03b65616803e9939240e))
* use vnc since it bundled chrome ([ec71046](https://www.github.com/gengjiawen/monkey-rust/commit/ec710469ec7f0ea05e4b5057758f2e0056c7f677))

## [0.8.0](https://www.github.com/gengjiawen/monkey-rust/compare/v0.7.0...v0.8.0) (2021-09-18)


### Features

* auto publish cargo packages ([#71](https://www.github.com/gengjiawen/monkey-rust/issues/71)) ([8957ca1](https://www.github.com/gengjiawen/monkey-rust/commit/8957ca19cd6d2c7fa5addf282298cf2d21d4ca5d))
* move built-in to object ([1a40c1c](https://www.github.com/gengjiawen/monkey-rust/commit/1a40c1ce40b11d0427f22d7e8f690130a83fa9df))


### Bug Fixes

* compiler warning ([c74fde8](https://www.github.com/gengjiawen/monkey-rust/commit/c74fde88291ba44e32d94129d5fd02036c9a5764))
* fncall span issue ([07484b1](https://www.github.com/gengjiawen/monkey-rust/commit/07484b1b35a928553ba5d60e8227e8c821a7e702))
* interpreter description ([bee0533](https://www.github.com/gengjiawen/monkey-rust/commit/bee05332fa77ea5bfea07121c7f944b62d82affc))


### Dev

* add dev log ([2c6e9db](https://www.github.com/gengjiawen/monkey-rust/commit/2c6e9db8890db40601ca373f2b649bf3c0b1ba18))
* auto setup astexplorer ([37097ed](https://www.github.com/gengjiawen/monkey-rust/commit/37097eddc83f4fc513e25d471fd34cd1d54bed0c))
* remove redundant step ([dd68d9d](https://www.github.com/gengjiawen/monkey-rust/commit/dd68d9d649fdd2ef24d027cef73f0839fbd55e7c))
* setup astexplorer ([ef803aa](https://www.github.com/gengjiawen/monkey-rust/commit/ef803aa50241b46836341a39335920e7f3a4d842))
* split interpreter test ([ddf668e](https://www.github.com/gengjiawen/monkey-rust/commit/ddf668e321d82b4d2e3b4d98188278ad3bed7002))
* split parser test ([73801aa](https://www.github.com/gengjiawen/monkey-rust/commit/73801aa678b3489533b2a7e84571d9d71a779015))
* use latest node.js ([3bac409](https://www.github.com/gengjiawen/monkey-rust/commit/3bac4095a44357cf9eec4b2e685779e5e075df66))

## [0.7.0](https://www.github.com/gengjiawen/monkey-rust/compare/v0.6.0...v0.7.0) (2021-07-25)


### Features

* add comment support ([043e2cb](https://www.github.com/gengjiawen/monkey-rust/commit/043e2cb0db4c936a1ea932686e13528e5f7585e6))
* split lexer test ([2dc9b22](https://www.github.com/gengjiawen/monkey-rust/commit/2dc9b223399257d6c24532cdb93e96917df9f9c7))
* split object system ([0862556](https://www.github.com/gengjiawen/monkey-rust/commit/08625566ec404c3a6c40fafc8d32bf2cf3e0d418))


### Bug Fixes

* add missing keywords ([c685622](https://www.github.com/gengjiawen/monkey-rust/commit/c6856228d042d246daf5b85b1e4b1a655ee27d8c))

## [0.6.0](https://www.github.com/gengjiawen/monkey-rust/compare/v0.5.0...v0.6.0) (2021-07-19)


### Features

* add test case to astsexplorer ([c873b23](https://www.github.com/gengjiawen/monkey-rust/commit/c873b23f0f1e6960ecfd766e17bf78f784e4ca5e))
* put cargo-workspace into docker ([053bb27](https://www.github.com/gengjiawen/monkey-rust/commit/053bb276c0e32ccaed1a7024bbddf9006da705e1))
* refactor function params to IDENTIFIER type. ([89faebe](https://www.github.com/gengjiawen/monkey-rust/commit/89faebeff7e7e1a484b42f65daf18cae05d2ba34))
* refactor token output ([ae2084e](https://www.github.com/gengjiawen/monkey-rust/commit/ae2084e55fbca08e5b3667c4e947641744de0f1b))


### Bug Fixes

* change default release name ([ecd5e54](https://www.github.com/gengjiawen/monkey-rust/commit/ecd5e540119eb641b60ab5c6189241627d188ac7))
* if span issue ([0236a30](https://www.github.com/gengjiawen/monkey-rust/commit/0236a303dbd8ccaecef39d5f4c24681841aa794d))

## [0.5.0](https://www.github.com/gengjiawen/monkey-rust/compare/v0.4.2...v0.5.0) (2021-01-19)


### Features

* add ast-explorer ([#29](https://www.github.com/gengjiawen/monkey-rust/issues/29)) ([cc58d93](https://www.github.com/gengjiawen/monkey-rust/commit/cc58d939d04c509a53f38bf112bb97fe7ccef323))
* add fish to gitpods ([65587f7](https://www.github.com/gengjiawen/monkey-rust/commit/65587f783911c2bd81bf37152b187805f403449f))


### Bug Fixes

* add versions back ([197c89e](https://www.github.com/gengjiawen/monkey-rust/commit/197c89e644290d806e7d916889485ed9751c8478))
* binary expr span issues. ([c543942](https://www.github.com/gengjiawen/monkey-rust/commit/c5439420cd49417bf23dbec2debd872279463498))

### [0.4.2](https://www.github.com/gengjiawen/monkey-rust/compare/v0.4.1...v0.4.2) (2021-01-07)


### Bug Fixes

* remove unused interface ([31dad4e](https://www.github.com/gengjiawen/monkey-rust/commit/31dad4ef02ca27e5fe0a01a99ccd711cbaaccd50))
* use latest Node.js LTS for release ([13394bf](https://www.github.com/gengjiawen/monkey-rust/commit/13394bfd17b39ba4d282126b10b3ea939ee5578b))

### [0.4.1](https://www.github.com/gengjiawen/monkey-rust/compare/v0.4.0...v0.4.1) (2021-01-05)


### Bug Fixes

* add missing id in release job ([6afacf7](https://www.github.com/gengjiawen/monkey-rust/commit/6afacf7a5eb243312ae4d3d5e806342535c27a14))
* update crates to right version ([43c4977](https://www.github.com/gengjiawen/monkey-rust/commit/43c4977f5189f4f42bddbfed43b661ac3dfe1f88))

## [0.4.0](https://www.github.com/gengjiawen/monkey-rust/compare/v0.3.0...v0.4.0) (2021-01-02)


### Features

* add array and hash to ast tree ([9f981b5](https://www.github.com/gengjiawen/monkey-rust/commit/9f981b53b9734c4f59278cf8aa34ff7f5eb99653))
* add binary expression ast ([da1e7f7](https://www.github.com/gengjiawen/monkey-rust/commit/da1e7f799873914001437b92dfc86c6b717f092b))
* add circleci ([#19](https://www.github.com/gengjiawen/monkey-rust/issues/19)) ([e4a4946](https://www.github.com/gengjiawen/monkey-rust/commit/e4a494691e9fbf39cd15c56f9ab436d0f6b61782))
* add example ([578a336](https://www.github.com/gengjiawen/monkey-rust/commit/578a336fad1cb51f75ab106aab7c26e601890840))
* add function call ast ([55c3808](https://www.github.com/gengjiawen/monkey-rust/commit/55c3808d42611b9fd3ba592d7d599792553297b7))
* add function declaration ast ([0b50d79](https://www.github.com/gengjiawen/monkey-rust/commit/0b50d79633a25f827b92a56d195d420063aac930))
* add if expression ast ([b3f9e98](https://www.github.com/gengjiawen/monkey-rust/commit/b3f9e983853d414236bfed691f9e3b9a9a327ba1))
* add index ast ([aa31ae0](https://www.github.com/gengjiawen/monkey-rust/commit/aa31ae0a7db9b45584c0b9a2ef50352c6a762866))
* add initial wasm version ([f3e24d5](https://www.github.com/gengjiawen/monkey-rust/commit/f3e24d5138f81abf6f211a9f43ae67a122075937))
* add interface for program ast output ([d47687b](https://www.github.com/gengjiawen/monkey-rust/commit/d47687bfbc5412e89b3d92f102bb17b778ec670a))
* add let statement to ast tree ([ef30e9a](https://www.github.com/gengjiawen/monkey-rust/commit/ef30e9a9eafdec8661aedc6c979d201c72bd9c78))
* add license ([9aecc11](https://www.github.com/gengjiawen/monkey-rust/commit/9aecc11aa9217d6fecdb1339f2acdcbce8ee183e))
* add literal to ast ([54b4d03](https://www.github.com/gengjiawen/monkey-rust/commit/54b4d03adf1364f6190a38c234d38497335b818b))
* add location info to root node ([910619d](https://www.github.com/gengjiawen/monkey-rust/commit/910619d0522f66c7688b54ac2bf975f7b9caa5f8))
* add npm publish process to CI ([bab3ff1](https://www.github.com/gengjiawen/monkey-rust/commit/bab3ff197f9b04bd6420e5127c89f8f6aee5eb4e))
* add release it ([#15](https://www.github.com/gengjiawen/monkey-rust/issues/15)) ([03dfccd](https://www.github.com/gengjiawen/monkey-rust/commit/03dfccd12e2cec6a3f7ab81975899976def81602))
* add type to ast (not perfect solution, needs another process on js side) ([d4ff0bc](https://www.github.com/gengjiawen/monkey-rust/commit/d4ff0bc1c6aafa534c64ad2134bf350444fd925d))
* add unary expression ast ([aa6df7b](https://www.github.com/gengjiawen/monkey-rust/commit/aa6df7b0fff8bce584e509dc8cf67a01191c025d))
* add wasm init code ([#14](https://www.github.com/gengjiawen/monkey-rust/issues/14)) ([31cdc96](https://www.github.com/gengjiawen/monkey-rust/commit/31cdc96afe5951e5c9dae576fe552981c25af34c))
* fix return statement and finally fix type annotation ([a912818](https://www.github.com/gengjiawen/monkey-rust/commit/a912818b130d3cc998b11b8fddc181f84a0f14f9))
* refactor span to common data structure ([25f9dac](https://www.github.com/gengjiawen/monkey-rust/commit/25f9dacb04a592517528b107aa18847435b6c104))


### Bug Fixes

* change access to public ([aad6a58](https://www.github.com/gengjiawen/monkey-rust/commit/aad6a58b7203c3069dd45ee0d4a92ca047cb276f))
* fix release type ([ef76109](https://www.github.com/gengjiawen/monkey-rust/commit/ef76109c074faf6a258d0df9d0ff7aefafc5b9e5))
* interpreter name ([87f774a](https://www.github.com/gengjiawen/monkey-rust/commit/87f774a56489f509583dad8f0b456583a27f93bd))
* wasm-pack build ([edc8081](https://www.github.com/gengjiawen/monkey-rust/commit/edc8081cf2125f6873e039f9797f5c94664e6eec))
