# MCP-MCP Spike — ein MCP, der andere MCPs frontet

Beantwortet die Frage: **lässt sich ein MCP-Router (Kind-MCPs zur Laufzeit an-/abklemmen,
Tools aggregieren, Calls weiterleiten, dazu eine UI) in Varg bauen?**

**Antwort: ja** — nachdem sechs Compiler-/Runtime-Lücken geschlossen wurden, die der Spike
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
| `mcp_mcp_ui.varg` | Router + HTTP-Control-Plane (`/`, `/tools`, `/call`, `POST /detach`) |

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

UI (live über HTTP verifiziert):

```
GET  /tools   → echo.echo, math.double, math.square
GET  /call    → proxyt durch den Router bis ins Kind
POST /detach  → {"detached":"math"}
GET  /tools   → nur noch echo.echo          ← Hotswap per HTTP
```

Damit sind alle vier Kern-Eigenschaften belegt: **Aggregation**, **Forwarding**,
**Hot-Unplug zur Laufzeit**, **UI-Control-Plane**.

`math.double` mit `{"n":21}` → `42` ist der wichtigste Datenpunkt: hätte der Router die Argumente
als String-Map weitergereicht, käme `n="21"` an und `json_get_int` lieferte `0`. Die `42` beweist,
dass Argumente **typerhaltend** über den Hop gehen.

## Aufgedeckte und geschlossene Lücken

Der Spike war weniger „schreib den Router" als ein Test, ob Varg das überhaupt ausdrücken kann.
Sechs Dinge standen im Weg — alle im Compiler/Runtime gefixt, nicht im Spike umschifft:

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

## Bekannte Grenzen (nicht umschifft, sondern benannt)

- **Attach zur Laufzeit über die UI** geht noch nicht: ein Handler kann eine neue Verbindung öffnen,
  aber es gibt keine gemeinsame, mutierbare Registry, in der er sie ablegen könnte. Detach geht
  (der Handler hält den Server-Handle). Ein Varg-seitiges `Map<string, Handle>` wäre dafür nötig.
- **Route-Handler erreichen `self` nicht** (`Fn`-Closures). Deshalb wird die Seite einmal vorab
  gerendert und vom Handler gecaptured.
- Der Router registriert Kind-Tools mit fixem Namespace; ein echtes Produkt würde Konflikte,
  Schema-Merge und Kind-Neustarts behandeln.
