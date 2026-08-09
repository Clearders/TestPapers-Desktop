# Desktop shell IPC v1

All payloads use camelCase and `schemaVersion: 1`. Rust serde tests protect the wire shape; TypeScript parsers reject malformed payloads before application state is updated.

## Types

- `ThemePreference`: `system | light | dark`
- `CloseBehavior`: `ask | quit | tray`
- `CloseDecision`: `quit | tray | cancel`
- `ShellContext`: application version, platform, theme state, effective close behavior, tray/settings availability, and startup warnings
- `DialogPreviewResult`: preview kind, cancellation state, selection count, and display-name basenames only

## Commands

| Command | Arguments | Result |
| --- | --- | --- |
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
| `testpapers://shell/close-requested` | versioned, increasing `requestId` |
| `testpapers://shell/preferences-requested` | versioned empty shell event |
| `testpapers://shell/theme-changed` | theme preference and effective theme |
| `testpapers://shell/dialog-previewed` | `DialogPreviewResult` |

Adding or broadening IPC requires updating the command manifest, capability file, both language boundaries, tests, and this inventory. Arbitrary filesystem paths, SQL, shell execution, HTTP, and secrets are not valid shell IPC.
