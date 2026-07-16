# MCP-MCP Spike — ein MCP, der andere MCPs frontet

Beantwortet die Frage: **lässt sich ein MCP-Router (Kind-MCPs zur Laufzeit an-/abklemmen,
Tools aggregieren, Calls weiterleiten, dazu eine UI) in Varg bauen?**

**Antwort: ja** — nachdem acht Compiler-/Runtime-Lücken geschlossen wurden, die der Spike
aufgedeckt hat (siehe unten). Kein npm, kein Netz: die Kind-MCPs sind selbst Varg-Programme.

## Ausführen

```bash
VARGC=../../varg-compiler/target/release/vargc.exe ./run.sh        # Router-Demo (headless)
VARGC=../../varg-compiler/target/release/vargc.exe ./run.sh --ui   # Control-UI :8710
```

## Bestandteile

| Datei | Rolle |
|---|---|
| `child_echo.varg` | Kind-MCP, 1 Tool (`echo`) |
| `child_math.varg` | Kind-MCP, 2 Tools (`double`, `square`) mit **numerischem** Argument |
| `mcp_mcp.varg` | Router: attach → aggregate → forward → hot-unplug (deterministisch) |
| `mcp_mcp_ui.varg` | Router + HTTP-Control-Plane (`/`, `/tools`, `/call`, `POST /attach`, `POST /detach`) |

## Was der Spike beweist

Router-Demo:

```
attached: 2 children
aggregated tools: 3
has math.double: true
echo.echo   -> ... "echo-mcp says: {\"msg\":\"hi\"}"
math.double -> ... "42"          ← n=21 kommt als ZAHL an, nicht als "21"
math.square -> ... "49"
detached: math
aggregated tools: 1
has math.double: false
echo.echo   -> ... "echo-mcp says: {\"msg\":\"still here\"}"   ← Nachbar lebt weiter
```

UI (live über HTTP verifiziert) — der Router startet **nur mit echo**, das math-Kind wird zur
Laufzeit an- und abgeklemmt:

```
start        → echo.echo
POST /attach → echo.echo, math.double, math.square   ← Kind zur Laufzeit gespawnt
GET  /call   → "42"                                   ← Forwarding durchs frische Kind
POST /detach → echo.echo
re-attach    → echo.echo, math.double, math.square   ← wiederholbar
```

Damit sind alle Kern-Eigenschaften belegt: **Aggregation**, **Forwarding**, **Attach und
Hot-Unplug zur Laufzeit** (wiederholbar, UI-getrieben) und die **UI-Control-Plane**.

`math.double` mit `{"n":21}` → `42` ist der wichtigste Datenpunkt: hätte der Router die Argumente
als String-Map weitergereicht, käme `n="21"` an und `json_get_int` lieferte `0`. Die `42` beweist,
dass Argumente **typerhaltend** über den Hop gehen.

## Aufgedeckte und geschlossene Lücken

Der Spike war weniger „schreib den Router" als ein Test, ob Varg das überhaupt ausdrücken kann.
Acht Dinge standen im Weg — alle im Compiler/Runtime gefixt, nicht im Spike umschifft:

1. **`McpConnection` war kein Handle.** Jeder andere zustandsbehaftete Handle der Runtime
   (Vector-Store, Workflow, MCP-*Server*) ist `Arc<Mutex<_>>`; die Client-Verbindung war ein nacktes
   Struct mit `&mut`. Aus einem Tool-Handler (`Fn` + Send + Sync) unbenutzbar — also genau das, was
   ein Router braucht. Jetzt Handle wie der Rest.
2. **`mcp_call_tool` konnte keine rohen Argumente weiterreichen.** Es nahm nur
   `HashMap<String,String>` und **stringifizierte jeden Wert** (`{"n":42}` → `{"n":"42"}`). Für einen
   Proxy tödlich. Neu: `ToToolArgs` akzeptiert Map **oder** rohes JSON-Objekt (verbatim).
3. **`return` in einem Lambda wurde `Ok(...)`-gewrappt**, wenn die *umgebende* Methode `?` benutzte —
   das Flag leckte in den Lambda-Body. Ein Lambda ist ein eigener Funktions-Scope.
4. **Handler-Closures konsumierten ihre Captures.** Zwei Tools desselben Kindes → „use of moved
   value". Captures werden jetzt in die Closure geklont (Handles sind `Arc`, also billig).
5. **`http_route`/`ws_route`-Handler konnten gar nichts capturen** — sie wurden als borrowende
   `|req| …`-Closure emittiert („closure may outlive the current function"). Web-Handler waren damit
   faktisch auf zustandslos beschränkt; die UI wäre unmöglich gewesen. Jetzt `move` + geklonte Captures.
6. **`foreach` verbrauchte die Kollektion** (`into_iter`), ein zweiter Durchlauf schlug fehl. Die
   Last-Use-Analyse des Codegens existierte bereits — `foreach` fragte sie nur nicht. Nebenbei fand
   sich, dass der Usage-Walker `or`, Lambda, Match, Interpolation u. a. **gar nicht besuchte**, also
   Verwendungen unterzählte — was auch die Move-vs-Clone-Entscheidung speist.
7. **Verschachtelte Lambdas verloren ihre Bindungen.** Ein Tool *aus einem HTTP-Handler heraus* zu
   registrieren (= Attach zur Laufzeit) scheiterte: der Parameter des inneren Lambdas wurde vom
   äußeren Handler als Capture behandelt und geklont (`let args = args.clone();` → „not found in
   this scope"). Gebundene Namen innerer Lambdas zählen jetzt korrekt nicht als freie Variablen.
8. **`try/catch` + `return` war generell kaputt** (nicht nur im Handler): der try-Body wird für
   `catch_unwind` in eine Closure gewickelt, also verließ ein `return` nur die Closure → Typfehler.
   Zusätzlich zählte der Typechecker `try/catch` gar nicht als returnend („not all code paths
   return"). Die Closure trägt den Rückgabewert jetzt heraus (`Ok(Some(v))`, Typ von Rust inferiert).
   Damit funktioniert im Handler auch **`?`**: es propagiert in die try-Closure → wird zum `catch`.

## Bekannte Grenzen (nicht umschifft, sondern benannt)

- **Route-Handler erreichen `self` nicht** (`Fn`-Closures) — deshalb wird die Seite einmal vorab
  gerendert und vom Handler gecaptured. Das bleibt eine echte Grenze: ein geklontes `self` hätte
  Snapshot-Semantik (`self.x = …` würde stillschweigend eine Kopie ändern), und ein geteiltes
  `Arc<Mutex<Agent>>` würde deadlocken, weil die umgebende Methode `self` schon hält. Statt das zu
  faken, **lehnt der Typechecker es jetzt mit einer brauchbaren Meldung ab** statt es als rustc-
  Fehler („cannot borrow `*self` as mutable…") durchzureichen.
- Der Router registriert Kind-Tools mit fixem Namespace und kennt nur das math-Kind beim Attach;
  ein echtes Produkt würde Kind-Liste, Namenskonflikte, Schema-Merge und Neustarts behandeln.
