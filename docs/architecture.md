# Desktop shell architecture

CLE-23 establishes a presentation shell and native boundary without implementing application business features.

## Dependency direction

```text
Vue components
  -> application/useDesktopShell
    -> infrastructure/tauri/shellBridge
      -> allowlisted Tauri commands and events

Rust ipc
  -> application
    -> domain
  -> infrastructure (settings, dialogs, native UI)

sync and migrations
  -> boundary documentation only in CLE-23
```

Vue components never import `invoke` or Tauri event APIs. `shellBridge.ts` owns invocation names, argument shapes, event subscriptions, and runtime DTO guards. Rust commands translate IPC into application operations; the domain contains preferences and the close state machine; infrastructure owns native window/menu/tray/dialog and settings-file details.

## Runtime lifecycle

The sole `main` window is 1180×760 with an 880×600 minimum and starts hidden. Vue subscribes to native events, loads `ShellContext`, applies the effective theme, and then calls `frontend_ready`; Rust shows and focuses the window. Shutdown marks explicit quit, flushes preferences, drops frontend listeners through Vue unmount, and runs Rust cleanup once.

Settings are non-sensitive JSON at the platform application-data location under `settings.v1.json`. Missing settings use defaults. Invalid or unreadable settings report a startup warning and use defaults; an unavailable or unwritable store degrades to session state and is reported through `ShellContext`.

The default close behavior is `ask`. Only one close request can be pending, request IDs increase, and stale or duplicate resolutions are rejected. Choosing quit or tray persists that behavior; cancel leaves `ask` unchanged. A saved tray preference degrades to `ask` if the platform tray is unavailable.

## Security boundary

The main capability is local-only and scoped to the `main` window. It permits only seven custom commands plus event listen/unlisten. Global Tauri access and remote WebViews are disabled. There are no direct fs, SQL, shell, HTTP, Cloud-client, updater, or Local Engine dependencies. Dialog implementations retain native paths in Rust and serialize only basenames.
