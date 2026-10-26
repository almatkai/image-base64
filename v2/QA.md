# Verification and reproducibility

Own source HEAD: `317da99b58e2603444b3dfb9de7e97d6987f8ca7`.
Host object: `ea100cee3f69711de12485ba061950c5fc65258f`.
SDK object: `e955911f0bf5b558f9d3eb5183df0adc95c22af2`.
[qa/pins.json](qa/pins.json) hashes every frozen host/SDK fixture.

Recovery inspected the existing dirty clone and found no clone-owned active build
or QA. The recovered code was independently read, corrected, and retested. V1
tracked files remain byte-identical to HEAD, except for the root README link.
No staging, commit, push, PR, release, install, host edit, or clone deletion occurred.

All execution homes, selected files, Downloads, Cargo/npm caches, source copies,
build outputs, temporary files, and archives stay under `v2/.qa` or ignored
`v2/target`. The source oracle executes actual locked TSX/helpers in a closed VM.
Its fs producer supports only explicit synthetic selected reads and an inert
write callback; every other fs method (including sync/promises/streams) is denied.
os.homedir is synthetic before module evaluation. Clipboard/toasts are captures;
unknown requires, process, network, browser, settings, selection, and native APIs
are unavailable. Both successful and failed source download callbacks are invoked.
No real user Downloads, clipboard, dialogs, or foreground state are used.

Reproduce from the clone root:

```sh
python3 v2/qa/verify.py
python3 v2/qa/fresh-build.py
python3 v2/qa/package.py
```

The first command prepares disposable exact-lock V1/renderer dependencies under
`.qa`, regenerates the oracle/host test excerpts, and runs both toolchains, strict
all-target Clippy, fmt, SDK pipe smoke, pinned renderer and schema tests. It also
attempts Windows GNU compilation if that target is already installed. Native
Windows MSVC tools are not installed by QA.

Coverage includes source syntax/padding/unpadded input, all-quote stripping,
retained arbitrary data-URI prefixes, UTF-16 sizes and forgiving Buffer decoding,
PNG offsets, JPEG traversal, WebP/VP8L helper quirks, truncation/nonimage fallback,
all three output labels without transcoding, source action ordering/payloads,
file read failures, drafts/failed submit/Convert Another, A-B-A revisions,
stale one-use tokens, closed clipboard ACKs, cancellation/late replies/EOF,
140 immediate canceled-open cycles, capacity/recovery, malicious names/previews,
permission failures, injected publication failure, all 16 collision candidates,
byte-exact destination staging and descriptor-relative cleanup. Both commands
exercise Copy success → fresh normal Copy → second explicit copy, stale IDs and
refresh without RPC, plus invalid/canceled/late ACK exact-text recovery. Production
pipes also cover failed Download → consumed stale ID → fresh retry, successful
Download with an unusable captured SDK temp path, and Download checkpoints across
repeated copies. No volumes are mounted; actual cross-volume execution is unrun.

`host-contract.py` compiles exact pinned/proposed registry and installer excerpts,
including the actual ZIP extractor (traversal/absolute/reserved names, case collisions,
symlink entries and the real dev ZIP), closed inventory verification, tampering, aliases, active resource
roots, and native launch ID selection. Unrelated launcher contributions are
inert and asserted absent. `producers.py` compiles the proposed native picker body
(with only its Tauri macro removed) and exact clipboard producer against inert OS
implementations. Generated tests are retained as Git-visible source, reproducible with rustfmt.
Two narrow Clippy allowances preserve frozen host expressions verbatim; product
code has no lint exemption.

`renderer.cjs` mounts actual pinned FormRuntime, DetailRuntime, Markdown,
ExtensionRuntimeSurface and ExtensionRuntimeView in React/JSDOM, before and after
the patch. It exercises actual executable-generated previews plus genuine synthetic
PNG/JPEG/WebP/lossless WebP fixture bytes. Native producers and timers are inert;
resource loading is denied. JSDOM assertions establish the image src contract,
not native image decoding or visual/GUI acceptance. React/Markdown dependencies
match the exact pinned host lock versions. `schema.cjs` uses the actual draft-2020-12
schema, including URI validation and 14 negative cases.

`fresh-build.py` copies final product source/lock into a new owned directory and
builds with a new CargoHome over HTTPS. `package.py` creates an ignored ARM64 dev
ZIP, verifies a closed SHA256 inventory and safe extraction, then runs the full
actual SDK pipe suite from the extracted executable. Its manifest still has no
release artifact pins; it cannot be installed by the pinned host. The host's
artifact rejection is intentional. No fabricated release URL/catalog entry exists.

Excluded acceptance: live Tauri build, actual file dialog, native clipboard,
real home Downloads, GUI rendering/image decode, macOS launch/import/install,
Windows execution/linking/MSVC, abrupt process-death durability, and hostile
concurrent filesystem replacement on Windows. Header-only preview recognition
and in-process checkpoints have the limits described in README.

XERJ used only the canonical clone prefix `ax-c1bea3cdd4b3`. Existing frozen-schema
and HTTP 429/RSS failures prevented complete indexing. Recovery reindexing again
hit the 768 MiB watermark/429 and was interrupted (its own PID only); lexical
search worked with incomplete coverage. No prefix bypass, global restart, schema
change, or feedback was used. Full reads, Git object bytes, hashes, and exact rg
checks supplied authoritative fallback coverage.

Observed Windows limitation: GNU cargo check passes; the link attempt fails because
`x86_64-w64-mingw32-dlltool` is absent. The MSVC target/toolchain is unavailable.
Final XERJ indexing also fails immediately on the retained README family changing
from `txt-lines` to `txt-prose`; its schema was left untouched.
