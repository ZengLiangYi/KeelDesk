## Summary

<!-- Briefly describe the changes in this pull request and why they are needed. -->

## Related Issue

<!-- Link any related issue, e.g., Closes #12 -->

## Checklist

- [ ] **One-way architecture boundary preserved**: `crates/keeldesk-core/`, `crates/tauri-plugin-keeldesk/`, and `packages/react/` do not import or depend on `apps/env-workbench/` business modules.
- [ ] **Specta bindings synced**: Ran `pnpm bindings` if any Rust command or event changed, and did not hand-edit `packages/react/src/bindings.ts` or `apps/env-workbench/src/bindings.ts`.
- [ ] **The Graphite Spine & Windows 11 native chrome respected**: UI changes follow `DESIGN.md` / `PRODUCT.md` / `AGENTS.md` tokens, scales, `100dvh` scroll containment, and Win32 DWM titlebar rules (and preserve the `KeelDesk` brand name).
- [ ] **Checks pass locally**: Verified `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `pnpm typecheck`, `pnpm build`, and `cargo test --workspace`.
