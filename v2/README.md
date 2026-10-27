# Image Base64 Converter — Rust V2

Development-only standalone external process for `com.tezbar.native.image-base64`.
Both original commands are implemented: **Base64 to Image** and **Image to Base64**.
The original Raycast icon is copied byte for byte. This source is not an installable
release: `extension.json` deliberately has `artifacts: {}`. Nothing is published or installed.

Build with Rust 1.85 or newer:

```sh
cargo build --locked --release --manifest-path v2/Cargo.toml
```

The HTTPS SDK dependency is exactly `tezbar-extension-sdk` 0.1.1 at
`e955911f0bf5b558f9d3eb5183df0adc95c22af2`. Product execution uses Rust and JSONL
protocol 1/schema 3, without a JavaScript runtime or compiled host adapter.

The Form → Detail → Convert Another flow preserves drafts, metadata, action order,
format selection, and source helper quirks. PNG/JPEG/WebP selection changes the
label and download extension; **it does not transcode bytes**. Copy writes the source
text data URI. Download explicitly writes the original forgivingly decoded payload.

The pinned host requires the **unapplied** generic
[integration patch](integration/host-prerequisites.patch). It supplies a native
single-file picker, all-field revision/token handling, late-response fencing,
Detail button event wrapping, verified replacement-ID routing, and active-version
resource lookup separate from canonical preferences. Before/after tests execute
pinned code excerpts and actual React components. This patch has not been applied
to the app, and a complete Tauri build/GUI/native dialog check has not run.

Behavioral differences and limits:

- Maximum input: 1,500,000 UTF-8 bytes; decoded/selected file: 1,000,000 bytes;
  16 sessions; one selected regular absolute file; path ≤4096 bytes; filename
  ≤512 UTF-8 bytes. Oversized inputs fail explicitly without truncation.
- Symbolic links and Windows reparse paths are rejected. Unix opens every path
  component with descriptor-relative no-follow operations; metadata is rechecked
  before/after the bounded read. Windows has compile-only coverage, not native
  race/permission acceptance. Non-UTF-8 filenames are rejected.
- Preview uses a separate canonical raster data URI based on the decoded signature.
  It never puts arbitrary MIME, filenames, SVG, HTML, or source Markdown into the
  renderer. Unknown/truncated payloads and excessive image dimensions use inert
  text; conversion, source copy, and raw download remain available. Preview limits
  are 8192 per side and 16,000,000 pixels. This is header inspection, not a full
  image validator; actual browser decoding is outside the synthetic QA.
- Downloads use `~/Downloads/image_<milliseconds>.<format>`, with `_1` through `_15`
  for collisions. An owned staging file in Downloads (0600 on Unix) is synced and
  hard-linked atomically without replacement; creation, publication, and cleanup
  use the validated directory handle on Unix. SDK temp is unused by Download and
  may reside on another volume or be unavailable. Downloads must exist, be
  symlink-free, and support hard links; unsupported filesystems fail explicitly
  and permit a fresh-token retry. Windows coverage is compile-only; native race
  guarantees and real cross-volume execution have not been verified.
  Original Raycast overwrote a colliding timestamp. Existing files are preserved.
- Action checkpoints live in bounded in-memory adapter state outside SDK transactional
  state. Copy/download IDs are consumed; old IDs cannot replay effects on refresh,
  stale actions, or Convert Another. Each successful Copy renders the normal Copy
  action with a fresh ID, allowing another deliberate copy of the same frozen URI.
  An acknowledged Download survives repeated copies without rewriting. Unknown
  copy outcomes expose **Retry Copy
  Exact Base64**, using frozen text without rereading. A dropped/canceled/late ACK
  cannot claim success. No crash-persistent journal or exactly-once OS guarantee is made.
- The source's text-file drag/drop hint is omitted: this port provides the text area
  and explicit native file picker. Metadata tag colors and Raycast built-in action
  icons are renderer adaptations, not additional image assets.

See [QA.md](QA.md), [NOTICES.md](NOTICES.md), the machine-readable
[verification evidence](qa/evidence.json), and [file hashes](INDEX.sha256).
