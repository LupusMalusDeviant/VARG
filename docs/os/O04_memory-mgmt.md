# O04: Memory Management & VRAM Multiplexing

Das teuerste Kapital eines AI-OS ist nicht der RAM, sondern der **VRAM (Video-RAM)**. Herkömmliche OS lagern RAM auf Festplatten aus (Swap). Das AI-OS muss VRAM, RAM und SurrealDB koordinieren.

## 1. Hybrides Model-Multiplexing
Nicht jeder Agent kann sein eigenes Llama-3-Modell in den VRAM laden.
* **Foundation Models:** Große Sprachmodelle werden einmal vom `VramManagerAgent` im VRAM platziert. Anfragen aller User-Agenten werden gequeued, gemultiplext und in Batches (Continuous Batching) über die GPU gejagt.
* **Small Models / Adapters:** Kleine LoRA-Adapter oder Embedder-Modelle können dynamisch dazu geladen und wieder gelöscht werden.

## 2. Zero-Copy Context Sharing
Wenn Agent A ein 128k Token-Dokument gelesen hat und Agent B dazu befragt, darf dieser Text nicht kopiert werden.
* Das OS nutzt Ringpuffer und verlinkte Speicheradressen.
* Agent B erhält einen Read-Only Pointer (via Varg Semantik) auf den Kontext von Agent A. VRAM-Verbrauch steigt um 0 Byte.

## 3. Hibernation & Context Eject
Wenn das Context-Window des LLMs oder der VRAM voll läuft:
1. **AutoEject:** Alte Teile der Konversation (`Context`) werden vom `MemoryManager` aus dem RAM gelöscht und als Vektorgraph in SurrealDB gesichert.
2. **Hibernation:** Ein kompletter Agent, der auf User-Input wartet, wird eingefroren. Sein gesamter State wird zu einem Node in SurrealDB.
3. **Rehydrierung:** Kehrt der User zurück, lädt das OS den State anhand der Session-ID (Vektor-Query) in Mikrosekunden zurück in den RAM.
