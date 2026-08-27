# Releasing

Pushing a `v*` tag builds three platform archives, packages the editor extension, runs the
packaged compiler against a program it has never seen, and drafts a release. It does not publish
one: the draft is attached and a person decides.

```bash
git tag -s v2.3.0 -m "Varg v2.3.0"
git push origin v2.3.0
```

## What is verified today

- Each archive is built from a clean checkout of the tagged commit.
- Every archive ships a `.sha256`. Both installers and `vargc upgrade` check the download against
  it and refuse to install on a mismatch.
- After packaging, the archive is extracted somewhere else entirely and used to build and run a
  program. This exists because v2.2.0 shipped a compiler that could not build anything: the
  release test ran inside the source checkout, where the crates it needed happened to be present.
- GitHub attests the build provenance of every archive. Anyone can check which workflow, at which
  commit, produced a file they downloaded:

```bash
gh attestation verify varg-v2.3.0-windows-x64.zip --repo LupusMalusDeviant/VARG
```

## What is not verified, and what it would take

### The binaries are not signed

Windows SmartScreen warns on first run of an unsigned executable, and macOS Gatekeeper refuses one
outright unless the user clears it by hand. Neither can be fixed from CI alone; both need a
certificate that belongs to a person or an organisation.

**Windows.** The release workflow already carries the signing step. It is skipped while no
certificate is present, so nothing changes until two repository secrets exist:

| Secret | What goes in it |
|--------|-----------------|
| `WINDOWS_CERT_PFX` | The `.pfx`, base64-encoded: `certutil -encode cert.pfx cert.txt`, then paste the body without the BEGIN/END lines |
| `WINDOWS_CERT_PASSWORD` | The password for that `.pfx` |

The step then signs `vargc.exe` and `varg-lsp.exe` with SHA-256 and a timestamp, before packaging,
so the archives contain signed binaries. A code-signing certificate comes from a CA
(DigiCert, Sectigo and others); an OV certificate still shows a SmartScreen prompt until the
binary builds reputation, an EV certificate does not.

**macOS.** Not written, because it needs more than a secret: an Apple Developer ID Application
certificate, a notarisation submission to Apple after signing, and a stapled ticket. When those
credentials exist, the step belongs beside the Windows one and needs `codesign`, `xcrun
notarytool submit --wait`, and `xcrun stapler staple`.

**Linux.** Nothing to sign — the archive's checksum and the provenance attestation are what a
Linux user checks.

### The tag signature is reported, not required

`git verify-tag` runs in the release job and writes a warning when the tag is unsigned. It does
not fail the job: signing happens on the machine that creates the tag, so CI can only observe it.
The v2.2.0 tag was unsigned, which meant nothing tied that release to a person.

To sign tags, once:

```bash
git config --global user.signingkey <key-id>
git config --global tag.gpgsign true
```

`gpg.format ssh` with an SSH key works too, and GitHub verifies both if the public key is on the
account.

### `curl | bash` remains the documented install

The scripts verify what they download against a published checksum, which closes the tampering
window after the redirect. It does not close the first one: the reader is trusting this repository
before running anything. Someone who would rather not can download the archive, check its
`.sha256`, verify its attestation with `gh attestation verify`, and unpack it by hand — every step
of the install script is one of those three.

## Checklist before tagging

- `cargo test --workspace --features full`
- `bash golden/run.sh` and `bash probes/run.sh`
- `python docs-check/check.py` — the version in the docs comes from `Cargo.toml`, so bump that first
- `bash security/advisories.sh`
- `python benchmarks/run_all.py` if anything touched code that the performance tables quote; the
  doc gate compares the READMEs against `benchmarks/results.json`
- Update `CHANGELOG.md`
