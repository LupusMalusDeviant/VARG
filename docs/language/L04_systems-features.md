# L04: Systems-Programming Features

Da Varg das Framework (AI-OS) selbst kompilieren muss (Bootstrapping), benötigt die Sprache hardwarenahes Zugriffsmanagement. Reguläre KI-Sprachen verwehren dies, C/C++ setzen es voraus. Varg separiert es in "Unsafe Spaces".

## 1. Unsafe Blocks
Ähnlich wie in Rust können Speicheradressierungen explizit in `unsafe` Blöcke ausgelagert werden.
* Kein regulärer `agent` darf `unsafe` nutzen. Nur ein `system agent` darf es.
* Pointer-Arithmetik: `*mut T` und `*const T` Datentypen.
* Direkter RAM-Zugriff zur Manipulation von VRAM-Sektoren.

```csharp
system agent VramManager {
    public void Allocate(ulong size) {
        unsafe {
            // Pointer arithmetic to partition GPU memory
            byte* vram_ptr = OS.GetVramBase();
            // ...
        }
    }
}
```

## 2. Foreign Function Interface (FFI)
Varg muss direkt mit Linux (Kernel Syscalls) oder CUDA (Treiber) kommunizieren.
* `extern "C"` Blöcke für C-ABI Calls.
* `import native "libcuda.so"` für Shared Library Bindings.
* Primitive Interop-Types: `c_int`, `c_voidptr`.

## 3. Interrupt Handler
Ein spezielles Modifikator-Konzept für latenzsensible Kernel-Aufgaben.
* Wenn markiert als `interrupt`, garantiert der MLIR CodeGen latenzfreie Ausführung, ohne Actor-Message-Queue.
* Preemption: Ein laufender `interrupt` pausiert alle Ring 3 Agenten auf dem jeweiligen Kern sofort.

```csharp
[Interrupt(Irq = 14)]
public system void HandleGpuInterrupt() {
    // Immediate execution
}
```
