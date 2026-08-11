# Offline PDF engine release payload

Release packaging places the following audited files in this directory:

- `tectonic.exe` on Windows, or `tectonic` on the target Unix platform;
- `testpapers-bundle.ttb`, a minimal Tectonic bundle containing only the TeX packages and fonts
  exercised by the TestPapers export fixtures;
- `release.v1.json`, containing lowercase SHA-256 values named `binarySha256` and
  `bundleSha256`;
- the Tectonic license and the generated bundle's complete TeX Live/font license and source
  notices.

The Rust adapter refuses to run if any payload is missing or its checksum differs. It invokes
Tectonic with `--only-cached`, `--untrusted`, an empty environment apart from the explicit
untrusted-mode guard, a bounded timeout, and no system or network fallback.

Do not commit the upstream default bundle: Tectonic 0.16.9 resolves its v33 default to the full
multi-gigabyte TeX Live archive. Produce a reviewed minimal `.ttb` from the official Tectonic
bundle tooling and distribute the binary payload through the signed release artifact pipeline.
