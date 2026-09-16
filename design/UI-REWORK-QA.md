# RAM workspace rebuild

## Implemented architecture

- Workspace navigation replaces the old toolbar: accounts, server browser, transfer, groups, settings.
- Central account library adds explicit checkboxes, select-all, range selection, bulk refresh/removal, and a visible row-actions entry point.
- Session panel contains launch controls, presets, selected-account context, and account details.
- Shared native HTML dialog provides top-layer rendering, focus containment, Escape dismissal, and focus restoration for account, advanced launch, server, utilities, transfer, settings, group, preset, and removal dialogs.
- Saved theme is applied at startup and after saving settings.
- Watcher occupies layout space rather than covering the account table.

## Verification performed

- `npm run build`: passed (TypeScript + Vite).
- `npm run lint`: passed.
- `cargo check --manifest-path src-tauri/Cargo.toml`: passed with eight existing dead-code warnings.
- Browser fixtures: 12 synthetic accounts; no production credentials used.
- Desktop 1600 x 900, minimum 900 x 500, narrow 390 x 844: no document-level horizontal overflow. Account table retains its own horizontal scroll on compact windows.
- Verified account checkboxes, select-all, empty search state, modal open/Escape close, removal cancellation, group modal, advanced launch, save-preset modal, watcher expansion, and saving light theme.
- Manually reviewed desktop, compact, light-theme, add-account and transfer screenshots.

## Remaining limitations

- Native WebView2 runtime interaction was not automated. Rust checking and browser QA are not an end-to-end Tauri launch test.
- Real Roblox login, launch, account mutation, and import/export operations were not executed against live accounts.
- Manual drag reordering and bulk group assignment are not implemented.
- Styles are organized across base tokens, workspace/shell rules, and existing feature styles; further consolidation remains possible.
- Backend `LaunchOptions.launch_args` is currently reported unused by Rust; correcting frontend argument position does not establish backend support for extra launch arguments.
