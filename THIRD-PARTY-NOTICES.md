# Third-party notices

rbxts-rs is MIT-licensed (`LICENSE`). It is a port of roblox-ts and of packages roblox-ts uses, it bundles some of
their files, it builds on tsrs, a port of TypeScript, and it compiles in Luau's compiler. This file lists those works,
what rbxts-rs takes from each, and their licenses. The Rust crates rbxts-rs depends on through Cargo (crates.io and
git dependencies) are not copied into this repository; each carries its own license (`cargo metadata` lists them).
Two of them are listed here too, because every binary contains their code and their notices go with it: tsrs and
Luau (through `luau0-src`).

## roblox-ts

- **Source:** <https://github.com/roblox-ts/roblox-ts>, version 3.0.0 (master at 68648b0).
- **Used in:** `crates/rbxts_transform` (a port of `src/TSTransformer`, the diagnostics' texts in
  `src/Shared/diagnostics.ts` and the build pipeline of `src/Project`), `crates/rbxts_project` (`src/Project`'s
  setup), `crates/rbxts_cli` (`src/CLI`); `crates/rbxts_transform/include/RuntimeLib.luau` is roblox-ts's runtime
  library, bundled as it is (with roblox-ts's license beside it). Test fixtures hold sources from roblox-ts's tests
  project (`crates/rbxts_transform/tests/fixtures/loops/src/shared/loop.spec.ts`) and outputs stock roblox-ts wrote
  (the `expected/` folders and `expected-stdout.txt` files).
- **License:** MIT.

```text
MIT License

Copyright (c) 2018-2026 roblox-ts

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## @roblox-ts/luau-ast, @roblox-ts/path-translator, @roblox-ts/rojo-resolver

- **Source:** the npm packages `@roblox-ts/luau-ast` 2.1.2, `@roblox-ts/path-translator` 1.1.1 and
  `@roblox-ts/rojo-resolver` 1.3.0 (<https://github.com/roblox-ts>).
- **Used in:** `crates/rbxts_luau` (luau-ast: the Luau syntax tree, its builders and renderer),
  `crates/rbxts_project/src/path_translator.rs` (path-translator) and `crates/rbxts_project/src/rojo.rs`
  (rojo-resolver).
- **License:** MIT, the same text as roblox-ts's above, each package with the copyright line:

```text
Copyright (c) 2022 roblox-ts
```

## roblox-lua-promise

- **Source:** roblox-lua-promise by Eryn L. K. (evaera), as roblox-ts bundles it in its `include` folder.
- **Used in:** `crates/rbxts_transform/include/Promise.luau`, bundled as it is (the runtime include rbxtsc copies into
  a project when no roblox-ts package is installed).
- **License:** MIT, the same text as roblox-ts's above, with the copyright line:

```text
Copyright (c) 2019 Eryn L. K.
```

## yargs and yargs-parser

- **Source:** the npm packages `yargs` 18.2.0 and `yargs-parser` 22.0.0, as roblox-ts's CLI uses them.
- **Used in:** `crates/rbxts_cli/src/yargs.rs`, a port of the parts of both that roblox-ts's `rbxtsc` reaches.
- **License:** yargs: MIT, the same terms as roblox-ts's above, with the copyright line:

```text
Copyright 2010 James Halliday (mail@substack.net); Modified work Copyright 2014 Contributors (ben@npmjs.com)
```

yargs-parser: ISC.

```text
Copyright (c) 2016, Contributors

Permission to use, copy, modify, and/or distribute this software
for any purpose with or without fee is hereby granted, provided
that the above copyright notice and this permission notice
appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES
OF MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE
LIABLE FOR ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES
OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS,
WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION,
ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
```

## Node.js

- **Source:** <https://github.com/nodejs/node>.
- **Used in:** `crates/rbxts_project/src/nodepath.rs`, Node's `path` module (`win32` and `posix`) as the ported
  packages rely on it; `crates/rbxts_cli/src/locale_time_table.rs`, the time formats Node v23.10.0 prints per locale
  (`toLocaleTimeString`, from its ICU 76.1 with CLDR 46.0 data; see Unicode below).
- **License:** MIT: <https://github.com/nodejs/node/blob/main/LICENSE>, with the copyright lines
  "Copyright Node.js contributors. All rights reserved." and, for the parts from the joyent/node repository,
  "Copyright Joyent, Inc. and other Node contributors. All rights reserved."

## Unicode CLDR (through Node's ICU)

- **Used in:** `crates/rbxts_cli/src/locale_time_table.rs`: per-locale time formats derived from CLDR 46.0 data.
- **License:** Unicode License v3, "Copyright © 2016-2025 Unicode, Inc.":
  <https://www.unicode.org/license.txt> (also in Node's `LICENSE`, under ICU).

## tsrs

- **Source:** <https://github.com/oxc-project/tsrs>, a Rust port of TypeScript 7; rbxts-rs depends on a fork of it
  (<https://github.com/Velover/tsrs>: visibility flips only, pinned by rev in `Cargo.toml`; `docs/tsrs.md`, "The fork").
- **Used in:** every crate that reads TypeScript, as a Cargo git dependency (not copied into this repository).
  Diagnostic texts rbxts-rs prints come from it.
- **License:** Apache License 2.0: the `LICENSE` file of the tsrs repository
  (<https://www.apache.org/licenses/LICENSE-2.0>). Its `NOTICE` file begins:

```text
tsrs
Copyright 2026 Max Schwenk and the tsrs contributors

tsrs is a Rust port of the TypeScript 7 type checker (the Go implementation in microsoft/TypeScript, `tsc/internal`).
It is a derivative work of TypeScript, Copyright (c) Microsoft Corporation, which is licensed under the Apache
License, Version 2.0 (see LICENSE); the bundled lib.*.d.ts files and the structure of the code come from that
project. The tsrs binary embeds those lib.*.d.ts files. TypeScript's notices for third-party material follow,
unchanged from TypeScript's NOTICE.txt.
```

Parts of `tsrs_core` derive from code by The Go Authors (`gocompat`'s tables from Go's `strconv`, and `glob`); they
are under Go's BSD-style license, the tsrs repository's `licenses/Go-LICENSE`:

```text
Copyright 2009 The Go Authors.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

   * Redistributions of source code must retain the above copyright
notice, this list of conditions and the following disclaimer.
   * Redistributions in binary form must reproduce the above
copyright notice, this list of conditions and the following disclaimer
in the documentation and/or other materials provided with the
distribution.
   * Neither the name of Google LLC nor the names of its
contributors may be used to endorse or promote products derived from
this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

A binary built from rbxts-rs includes tsrs, so a distributed binary carries tsrs's `LICENSE`, its full `NOTICE`
(which includes TypeScript's notices for third-party material) and `licenses/Go-LICENSE`.

## TypeScript

- **Source:** <https://github.com/microsoft/TypeScript>, Copyright (c) Microsoft Corporation.
- **Used in:** `crates/rbxts_cli/src/pretty.rs` (a port of TypeScript 5's `formatDiagnosticsWithColorAndContext`,
  which stock roblox-ts prints with), `crates/rbxts_transform/src/util/ts5_property_order.rs` and
  `crates/rbxts_probe/src/property_order.rs` (TypeScript 5's property order), and TypeScript's diagnostic message
  texts in test fixtures (stock roblox-ts's output, `expected-stdout.txt`).
- **License:** Apache License 2.0: <https://www.apache.org/licenses/LICENSE-2.0> (the `LICENSE.txt` of the
  `typescript` npm package); third-party notices in its `ThirdPartyNoticeText.txt`.

## Luau

- **Source:** Luau's compiler (<https://github.com/luau-lang/luau>, release 0.740), from the sources of the
  `luau0-src` crate 0.22.0+luau740 (<https://github.com/mlua-rs/luau-src-rs>).
- **Used in:** `crates/rbxts_luau_check`, which builds Luau's compiler from those sources and links it in, so every
  `rbxtsc` binary contains it (`rbxts_transform` checks emitted Luau against Luau's limits with it).
- **License:** MIT, as the `luau0-src` crate's `LICENSE` gives it:

```text
MIT License

Copyright (c) 2022-2025 Aleksandr Orlenko (luau-src crate)
Copyright (c) 2019-2024 Roblox Corporation
Copyright (c) 1994–2019 Lua.org, PUC-Rio.

Permission is hereby granted, free of charge, to any person obtaining a copy of
this software and associated documentation files (the "Software"), to deal in
the Software without restriction, including without limitation the rights to
use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies
of the Software, and to permit persons to whom the Software is furnished to do
so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

A distributed binary carries this notice.
