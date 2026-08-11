# Desktop architecture

CLE-24 adds an embedded Rust Local Engine to the presentation shell and native boundary established by CLE-23.

## Dependency direction

```text
Vue components
  -> application/useDesktopShell
    -> infrastructure/tauri/shellBridge
      -> allowlisted Tauri commands and events

Rust ipc
  -> application (shell and Engine supervisor)
    -> domain
  -> infrastructure (settings, dialogs, native UI, workspace lease)

Local Engine worker
  -> workspace identity and OS file lease
  -> SQLite/data/jobs added behind this boundary by later CLE issues
```

Vue components never import `invoke` or Tauri event APIs. Dedicated bridges own invocation names, argument shapes, event subscriptions, and runtime DTO guards. Rust commands translate IPC into application operations; the domain contains preferences and lifecycle states; infrastructure owns native window/menu/tray/dialog, settings-file, and workspace-lock details.

## Runtime lifecycle

The sole `main` window is 1180×760 with an 880×600 minimum and starts hidden. Vue subscribes to native events, loads both contexts, applies the effective theme, and then calls `frontend_ready`; Rust shows and focuses the window. The Local Engine may still be starting or degraded when the window appears, so business views gate on `EngineContextV1` while recovery controls remain available.

Tauri's single-instance plugin focuses the primary window when the application is launched again. Independently, the Engine holds an OS file lease in the workspace for its entire ready lifetime and never guesses whether a PID file is stale. Startup runs on a supervised worker. A failed or panicking initializer is retried after 250 ms, 1 second, and 4 seconds; exhaustion enters `degraded`, and an explicit retry resets that budget. Every successful reconstruction increments `generation`.

Shutdown transitions the Engine to `stopping`, rejects recovery, interrupts startup backoff, waits up to five seconds for the worker, releases the workspace lease, flushes preferences, drops frontend listeners through Vue unmount, and runs Rust cleanup once.

Settings are non-sensitive JSON at the platform application-data location under `settings.v1.json`. Missing settings use defaults. Invalid or unreadable settings report a startup warning and use defaults; an unavailable or unwritable store degrades to session state and is reported through `ShellContext`.

The standard workspace is the `workspace` child of platform application data. `workspace.v1.json` holds stable UUIDv7 workspace and local-principal identities. It is written atomically while the workspace lease is held. Paths remain inside Rust and are never serialized over IPC.

The default close behavior is `ask`. Only one close request can be pending, request IDs increase, and stale or duplicate resolutions are rejected. Choosing quit or tray persists that behavior; cancel leaves `ask` unchanged. A saved tray preference degrades to `ask` if the platform tray is unavailable.

## Security boundary

The main capability is local-only and scoped to the `main` window. It permits an explicit command allowlist plus event listen/unlisten. Global Tauri access and remote WebViews are disabled. There is no generic fs, SQL, shell, HTTP, Cloud-client, or arbitrary command permission. The Engine is process-local and opens no network listener; typed IPC plus the window capability replaces a local HTTP token. Dialog and workspace implementations retain native paths in Rust and serialize only basenames or stable IDs.
