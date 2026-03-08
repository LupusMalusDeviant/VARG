# O05: OS Bootstrapping & Inception

Die ultimative Vision von Varg ist das **Bootstrapping**: Das point-in-time, ab dem der Compiler mächtig genug ist, dass wir die C/Rust-Basis verlassen können und das AI-OS in seiner eigenen Sprache (`.varg`) schreiben.

## Phase 6: Der Übergang (SurrealDB FS in Rust)
Bevor wir bootstrappen, binden wir SurrealDB in den Rust-Compiler als Backend ein. Das OS simuliert das Vektorgraph-Dateisystem, aber noch als reines C/Rust-Binary, das die `.varg` Scripte ausführt.

## Phase 7: OS in Varg Inception
In dieser finalen Phase werden die primären `system agent` Layer umgeschrieben:
1. **VramManagerAgent:** Geschrieben in Varg. Greift mittels `unsafe` Block und Linux C-ABI (`extern "C"`) direkt auf ROCm/CUDA-Treiber zu, um GPU-Speicherpages zu allokieren.
2. **FileSystemAgent:** Geschrieben in Varg. Öffnet den RocksDB-Store von SurrealDB direkt.
3. **NetworkStack:** Geschrieben in Varg. Horcht mittels Syscalls auf TCP-Sockets.

## Warum Bootstrapping?
Wenn das OS in Varg geschrieben ist, versteht die Sprache das OS, und das OS die Sprache. Eine LLM-Anfrage kann vom User-Agenten verlustfrei bis auf die VRAM-Hardware-Ebene verfolgt und optimiert werden, blockiert durch Compile-Time OCAP-Sicherheit. Es gibt keine schwarzen Boxen (wie unsaubere Python-Pipelines) mehr.
