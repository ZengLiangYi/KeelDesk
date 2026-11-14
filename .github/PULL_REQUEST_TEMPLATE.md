## Summary

<!-- Briefly describe the changes in this pull request and why they are needed. -->

## Related Issue

<!-- Link any related issue, e.g., Closes #12 -->

## Checklist

- [ ] **One-way architecture boundary preserved**: `src-tauri/src/keel_core/` and `src/keel-sdk/` do not import or depend on `workbench` modules.
- [ ] **Specta bindings synced**: Ran `pnpm bindings` if any Rust command or event changed, and did not hand-edit `src/bindings.ts`.
- [ ] **The Graphite Spine & Windows 11 native chrome respected**: UI changes follow `DESIGN.md` / `PRODUCT.md` / `AGENTS.md` tokens, scales, `100dvh` scroll containment, and Win32 DWM titlebar rules (and preserve the `KeelDesk` brand name).
- [ ] **Checks pass locally**: Verified `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `pnpm build`, and `cargo test --manifest-path src-tauri/Cargo.toml`.
