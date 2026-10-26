# Notices and provenance

The Rust port is MIT licensed; see [LICENSE](LICENSE). The original own V1
[license](../LICENSE) remains unchanged. Raycast extension code/icon provenance:
`raycast/extensions` commit `6734d85992290066fd31a7f5bee3cca3d3545a1a`,
`extensions/image-base64`; authentic [Raycast MIT license](resources/RAYCAST-LICENSE).

The 262,271-byte original icon SHA256 is
`3ad7436debd7aa7bedfae7f081e25d0f16ba554e8ad9414f663625adc1024a43`.
It matches the original pinned upstream URL, own `assets/extension-icon.png`, and
`resources/extension-icon.png` byte for byte. No command-specific image icons are
declared by either source manifest. No icon has been regenerated/recompressed.

The SDK 0.1.1 is pinned to `e955911f0bf5b558f9d3eb5183df0adc95c22af2`;
its authentic Apache 2.0 [license](resources/SDK-LICENSE) is retained.
Host QA fixtures are exact Git-show bytes at `ea100cee3f69711de12485ba061950c5fc65258f`,
with the original [host license](qa/fixtures/host/LICENSE). Fixture hashes and
upstream icon/license URL provenance are recorded in [pins.json](qa/pins.json).

Authentic Cargo dependency license/notice files are copied under
[resources/licenses](resources/licenses). [dependency-notices.json](qa/dependency-notices.json)
records exact locked versions, declared SPDX license expressions, source references,
and copied-file hashes; this includes development and platform-only dependencies.
Crates distributing no separate license file are explicitly visible as empty
notice lists, without invented copyright text. Cargo's registry source checksum
and declared license remain recorded. Third-party copyright notices are unmodified.

Synthetic 7×9 raster fixtures in `qa/fixtures/images` were created locally using
Pillow (PNG/JPEG/WebP and lossless WebP). They contain a single solid color, with
no third-party artwork. Small crafted ZIP fixtures in `qa/fixtures/archives` are
local inert parser-test bytes (including intentionally invalid names/link modes).
Renderer QA dependency versions are frozen separately in
`qa/package-lock.json`; they are development tools and never enter the product ZIP.
