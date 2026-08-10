# Manual desktop smoke test

Run on Windows, Ubuntu, and macOS using a clean application-data profile when validating a release candidate.

1. Launch the application and confirm the themed window appears without an unthemed flash.
2. Switch system/light/dark from both Vue and the native menu; confirm native checkmarks and Vue remain synchronized, including an operating-system theme change while `system` is selected.
3. Open Preferences from the application menu and tray; set each close behavior.
4. With `ask`, close the window and verify cancel, hide-to-tray, and exit. Confirm duplicate/stale actions do not trigger another transition.
5. Restore from the tray where the platform emits tray clicks; always verify Show/Hide menu actions and explicit Quit. On Linux, use the menu when click events are unavailable.
6. Preview CSV/JSON import and DOCX/TeX export. Test cancel and selection; confirm the UI shows only basenames and no content is read or written.
7. Restart and confirm theme/close preferences persist. Make the settings location unavailable and confirm the UI reports session-only fallback.
8. Explicitly quit and confirm the process disappears.

Automated smoke mode is available after `npm run build:desktop`:

```bash
node scripts/smoke-desktop.mjs
```

Linux CI runs the same command under Xvfb. Success requires `ready`, `cleanup`, and exit code 0.
