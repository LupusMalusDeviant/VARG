# Agent Control Plane (Wave 22b)

A dashboard for a Varg agent system, written in Varg and served by Varg.

```bash
vargc run dashboard/dashboard.varg
# [dashboard] http://127.0.0.1:8080
```

`VARG_DASHBOARD_ADDR` overrides the bind address.

## Why not SvelteKit

The design memo offered a SvelteKit frontend or "a Varg agent that generates HTML", and this is
the second. One native binary, no Node, no build step — and it is covered by the same golden and
probe suites as the rest of the repo, which a separate frontend project would not have been. The
cost is honest: the knowledge-graph panel draws hand-written SVG instead of using D3 or Cytoscape.

## Panels

| Panel | Source | Notes |
|---|---|---|
| Agents | `agents_list()` | Live status maintained by the generated dispatcher, not reported by the program |
| Traces | `trace_export()` | Span hierarchy, nested by `parent_id`, bars scaled by `duration_us` |
| Knowledge graph | `graph_query()` / `graph_neighbors()` | Nodes on a circle, edges labelled with their relation |
| Cost | `budget_*` | Real accounting of text this program actually processed, not placeholder numbers |

## How the live agent status works

The status is not something the program sets; it is maintained underneath it. Every `spawn`
registers the agent, and the generated dispatcher moves it through the states as it works:

```
register -> starting -> (on_start) -> idle -> running -> idle -> ... -> stopped
```

A handler that panics marks the agent `error` with the panic message and the agent keeps serving
the next message. That last part needed a change to the panic hook, which used to exit the whole
process from any thread — one bad message took the program down.

## The constraint that shapes the code

Route handlers compile to `Fn` closures and cannot reach `self`. So everything a handler serves is
either a captured runtime handle — those are `Arc`-based, so they stay live and the panels show
current data — or a value computed before the server starts. The HTML page is the latter: rendered
once at startup, captured, served from memory.

## Testing

The dashboard itself cannot be a golden program: it ends in `http_listen`, which blocks. What is
pinned instead is the contract the panels depend on — `golden/progs/dashboard_payloads.varg`
checks the shape of all four payloads. A payload that changes shape would otherwise break a panel
silently in the browser, where no test would notice.
