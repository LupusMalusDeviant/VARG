# O01: OS-Architecture (Privilege Rings)

Das AI-OS stellt die Kernel-Logik und die Orchestrierung der Agenten bereit. Entwickelt nach dem Paradigma des Bootstrappings, nutzen wir C/Rust nur für den nackten Hypervisor, die echte Logik wird im Varg Kernel-Space geschrieben.

## 1. Ring 0: System AgentSpace (Kernel)
Hier laufen Daemons, die kritische Ressourcen steuern.
* **Modifier:** `system agent`.
* **Berechtigungen:** Darf `unsafe` nutzen. Darf FFI importieren. Kann Interrupts registrieren. Generiert OCAP-Tokens, anstatt sie konsumieren zu müssen.
* **Lebenszyklus:** Laufen dauerhaft im Hintergrund, werden nie durch Hibernation aus dem RAM ejectiert.

**Beispiele:**
* `VramManagerAgent`: Multiplext Modelle, entscheidet wer VRAM erhält.
* `SurrealFsAgent`: Steuert `/sys/fs/surreal`, commited Graph-States im Hintergrund.
* `NetworkStackAgent`: Verwaltet TCP/IP Sockets.

## 2. Ring 3: User AgentSpace
Hier agieren die KI-Assistenten, die komplexe Denkarbeit leisten.
* **Modifier:** `agent`.
* **Berechtigungen:** Sandboxed. Keine OCAP Erzeugung, kein FFI.
* **Message Passing:** Können Kernel-Daemons um Ressourcen via Message anfragen (Actor Model).
* **Preemption & Hibernation:** User-Agenten sind der Willkür von Ring 0 ausgesetzt. Wenn RAM knapp wird, pausiert Ring 0 den Agenten sofort (Context Serialize -> DB).

## 3. OS-Architektur Prinzip
Das UI des OS (z.B. ein Dashboard analog zu Windows) ist lediglich ein Web/GraphQL-View, der den Status der Agenten aus SurrealDB abfragt. Das OS hat keine klassische Win32-Desktop-Umgebung, da Agenten über Protokolle (Matrix, Telegram, REST) in die echte Welt funken.
