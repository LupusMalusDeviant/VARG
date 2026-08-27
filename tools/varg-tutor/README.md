# varg-tutor

An MCP server, written in Varg, that teaches Varg.

The measured barrier to writing Varg is not that the language is hard — it is that no model has
seen it. Documentation alone does not fix that: guessing stays guessing until something answers.
So the first tool here is the compiler itself, in the loop.

## Tools

| | |
|---|---|
| `check(source)` | Type-check a snippet and return the compiler's own diagnostics. |
| `example(topic)` | A complete program for a topic, from the set CI builds and runs. |
| `builtins(query)` | Builtins with their signatures, from the gated reference. |

## Running it

```bash
vargc build tutor.varg

export VARG_HOME=/path/to/varg        # the checkout: golden programs and REFERENCE.md
export VARGC=/path/to/vargc           # optional; defaults to `vargc` on PATH
./tutor.exe --mcp-serve
```

Or, without an MCP client:

```bash
vargc mcp list tutor.varg
vargc mcp call tutor.varg check '{"source": "agent Main { public void Run() { print \"hi\"; } }"}'
```

On Windows, give `VARGC` a native path (backslashes) or leave it unset and put `vargc` on PATH:
`cmd.exe` will not take a quoted forward-slash path as a program name.

## Why it is written in Varg

Because it should be. It is 90 lines, it ships as one 590 KB binary with no runtime, and it starts
in about 5 ms — which is the argument for writing an MCP server in Varg, made by an MCP server
written in Varg.
