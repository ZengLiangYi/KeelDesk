# AGENTS.md

Guidance for agents working in this repository.

## Project Stack

- This project is a Tauri v2 desktop app with React, TypeScript, Vite, Tailwind CSS v4, shadcn/ui, and pnpm.
- Use pnpm for all JavaScript package operations.
- Do not introduce npm, yarn, `package-lock.json`, or `yarn.lock`.
- Keep `pnpm-lock.yaml` committed when dependencies change.

## Common Commands

```bash
pnpm install
pnpm tauri dev
pnpm bindings
pnpm build
pnpm tauri build
```

- `pnpm bindings` regenerates frontend TypeScript bindings from Rust.
- `pnpm build` runs `pnpm bindings` before `tsc` and `vite build`.
- Tauri's `beforeDevCommand` and `beforeBuildCommand` should continue to use pnpm.

## TypeScript And Tauri Commands

- Use `tauri-specta` for typed Rust-to-TypeScript command bindings.
- Do not hand-write frontend calls like `invoke("command_name", ...)` when a typed binding exists.
- Frontend code should import generated commands from `src/bindings.ts`:

```ts
import { commands } from "./bindings";
```

- `src/bindings.ts` is generated. Do not edit it manually.
- When adding a new Rust command:
  - Add both `#[tauri::command]` and `#[specta::specta]`.
  - Prefer owned command inputs such as `String` over borrowed inputs such as `&str`.
  - Register the command in `specta_builder()` via `collect_commands![...]`.
  - Run `pnpm bindings` and update frontend imports/calls.
- When adding Rust data types that cross the Tauri boundary, derive the relevant Serde traits and `specta::Type`.
- Keep Specta beta dependencies version-pinned with `=` unless intentionally upgrading the binding stack.

## Tailwind And shadcn/ui

- Tailwind CSS is configured through the Vite plugin `@tailwindcss/vite`.
- The global Tailwind/shadcn CSS entry is `src/index.css`.
- The import alias is `@/* -> ./src/*`.
- shadcn/ui uses the `radix-nova` style, Radix base, Lucide icons, and CSS variables.
- Add shadcn components with `pnpm dlx shadcn@latest add <component>`.
- Check `pnpm dlx shadcn@latest info --json` before assuming a component is installed.
- Generated shadcn component source lives in `src/components/ui`.
- Keep shared helpers in `src/lib`, especially `src/lib/utils.ts`.
- Prefer shadcn components over custom styled markup for common UI primitives.
- Use semantic tokens such as `bg-background`, `text-muted-foreground`, `border-border`, and `bg-card`.
- Use `gap-*` for spacing instead of `space-x-*` or `space-y-*`.
- Use `cn()` for conditional class composition.
- Use Lucide icons in buttons with `data-icon="inline-start"` or `data-icon="inline-end"`.
- Do not manually edit `components.json` unless the shadcn CLI cannot express the required change.
- Do not let old global CSS rules target all `button` or `input` elements; that will break shadcn variants.

## Window Chrome Rules

- This app uses a custom Tauri window chrome.
- `src-tauri/tauri.conf.json` should keep:
  - `decorations: false`
  - `transparent: true`
- The visual titlebar may be hidden, but a drag region must remain.
- Use `data-tauri-drag-region` for draggable areas.
- Put `data-tauri-drag-region="false"` on interactive controls inside draggable areas.
- Window controls should call Tauri window APIs from `@tauri-apps/api/window`.
- On Windows, the purple border is applied through DWM from Rust. Keep that logic in Tauri/Rust rather than trying to fake the outer OS border with CSS.
- Do not try to change the native Windows corner radius unless explicitly asked. It is mostly OS-managed.

## UI Direction

- Preferred visual direction: refined Fluent 2 inspired, quiet, precise, glassy where useful.
- Avoid cheap HUD layers, busy neon effects, and decorative clutter.
- Avoid hover scale effects on window controls and ordinary buttons.
- Prefer subtle state changes: color, opacity, shadow, border, or background.
- Window controls should feel intentional and polished, not plain text symbols dropped into buttons.
- Keep the transparent titlebar visually hidden while preserving drag behavior.
- Maintain the mysterious purple accent as the signature window edge treatment.

## Editing Rules

- Keep changes scoped to the requested behavior.
- Follow the existing React + CSS structure unless there is a clear reason to refactor.
- Use generated bindings for frontend/backend contracts.
- Prefer small, readable components over premature abstractions.
- Do not edit generated files manually.
- Do not overwrite unrelated user changes.

## Verification

Before claiming a change is complete, run the narrowest useful checks:

```bash
pnpm bindings
pnpm build
cargo check --manifest-path src-tauri/Cargo.toml
```

For visual/window changes, also run the Tauri app and manually verify:

- The app opens.
- The window can be dragged.
- Close, minimize, and maximize controls work.
- The transparent window and purple border still render correctly.
