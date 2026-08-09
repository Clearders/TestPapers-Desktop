# Web migration provenance

- Source repository: `Clearders/TestPapers`
- Source commit: `8f43db88166048ec29d80d3c364dda0cd609f994`
- Port date: 2026-08-09
- Destination issue: CLE-23

| Source | Destination | Port and differences |
| --- | --- | --- |
| `app/assets/css/main.css` | `src/styles/main.css` | Selected light/dark color, type, radius, and shadow tokens were copied. Nuxt page/layout rules, motion system, cookie theme state, SSR assumptions, and Web-only components were omitted; shell-specific layout rules were added. |
| `app/components/AppIcon.vue` | `src/components/AppIcon.vue` | The SVG renderer and geometry for the icons used by the shell were copied. Unused names were removed, Vue imports are explicit, and a parity test pins the Web `sparkles` geometry. |
| `public/favicon.ico` | `public/favicon.ico`, `src-tauri/icons/*` | The favicon was copied byte-for-byte for Vite and used as the source for Tauri desktop icon generation. |

No Web source file was changed. The source checkout was verified at the recorded commit and its worktree remained clean after the port.
