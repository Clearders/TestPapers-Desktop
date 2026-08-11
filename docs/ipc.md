# Desktop IPC v1

All payloads use camelCase and `schemaVersion: 1`. Rust serde tests protect the wire shape; TypeScript parsers reject malformed payloads before application state is updated.

## Types

- `ThemePreference`: `system | light | dark`
- `CloseBehavior`: `ask | quit | tray`
- `CloseDecision`: `quit | tray | cancel`
- `ShellContext`: application version, platform, theme state, effective close behavior, tray/settings availability, and startup warnings
- `DialogPreviewResult`: preview kind, cancellation state, selection count, and display-name basenames only
- `EngineState`: `starting | ready | recovering | degraded | stopping`
- `EngineErrorV1`: stable code, user-safe message, recoverability, and a suggested action
- `EngineContextV1`: Engine state/generation, nullable workspace UUID while starting, database and maintenance availability, and the last typed error

## Commands

| Command | Arguments | Result |
| --- | --- | --- |
| `get_engine_context` | none | `EngineContextV1` |
| `retry_engine_start` | none | immediate `recovering` context, or a typed `EngineErrorV1` when retry is not valid |
| `get_shell_context` | none | `ShellContext` |
| `frontend_ready` | none | none; shows the main window |
| `set_theme_preference` | `preference` | updated `ShellContext` |
| `set_close_behavior` | `behavior` | updated `ShellContext` |
| `resolve_close_request` | `requestId`, `decision` | versioned close outcome |
| `preview_question_import_dialog` | none | CSV/JSON `DialogPreviewResult` |
| `preview_paper_export_dialog` | `format: docx | tex` | target `DialogPreviewResult` |

## Events

| Event | Payload |
| --- | --- |
| `testpapers://engine/state-changed` | `EngineContextV1`; emitted for startup attempts and state transitions |
| `testpapers://workspace/maintenance-changed` | `EngineContextV1`; emitted when maintenance mode changes |
| `testpapers://shell/close-requested` | versioned, increasing `requestId` |
| `testpapers://shell/preferences-requested` | versioned empty shell event |
| `testpapers://shell/theme-changed` | theme preference and effective theme |
| `testpapers://shell/dialog-previewed` | `DialogPreviewResult` |

The Local Engine is embedded in the Tauri process. It does not listen on HTTP, allocate a port, or issue a bearer token. The local-only `main` window capability and command allowlist are the authentication and authorization boundary.

Adding or broadening IPC requires updating the command manifest, capability file, both language boundaries, tests, and this inventory. Arbitrary filesystem paths, SQL, shell execution, HTTP, and secrets are not valid Desktop IPC.
