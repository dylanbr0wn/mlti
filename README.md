---
tags: oss, rust, project, personal
---

# MLTI

MLTI is a concurrent process runner for the command line written in [Rust](https://www.rust-lang.org/). It is currently a straight forward port of the [concurrently](https://github.com/open-cli-tools/concurrently) package for node.js, but will be extended in the future once feature parity is reached.


## Installation

mlti is currently available through [npm](https://www.npmjs.com/package/mlti). To install mlti globally from npm:

```bash
# using npm
npm i -g mlti 
# using pnpm
pnpm i -g mlti
# using yarn
yarn global add mlti
```

Or, to install it as a development dependency for a project:

```sh
# using npm
npm i -D mlti 
# using pnpm
pnpm i -D mlti
# using yarn
yarn add -D mlti 
```

## Usage

mlti works the exactly like concurrently and already supports many of the same options.

```bash
mlti "echo hello" "echo world"
```

## Project Goals

Ultimately this is a project to help me learn threading in Rust but I do have long term goals to keep this project going.

- [ ] Feature Parity with concurrently
	- [ ] `--success` flag
	- [x] `--raw` flag
	- [x] `--no-color` flag
	- [ ] `--hide` flag
	- [x] `--group` flag
	- [ ] `--timings` flag
	- [ ] `--passthrough-arguments` flag
	- [ ] `--prefix-colors` flag
	- [x] `--timestamp-format` flag
	- [ ] npm/yarn/pnpm pattern matching 
- [ ] Explore adding shortcuts for other languages/package managers
- [x] Make available on npm package registry
- [ ] Make available through cargo
- [ ] Add a parallel/sequential flag
- [ ] Add shorthand combo aliases to combine flags
- [ ] Add an analysis flag that will output details on process performance after all or some are complete.
- [ ] Do some performance analysis against competing tools
