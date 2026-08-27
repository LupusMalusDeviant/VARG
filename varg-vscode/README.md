# Varg Language Support

Syntax highlighting and language-server support for [Varg](https://github.com/LupusMalusDeviant/VARG),
a compiled language for autonomous AI agents.

## What you get

- **Syntax highlighting** for `.varg` files — works on its own, no binary needed.
- **Diagnostics, hover and completion**, when `varg-lsp` is available. Go to Definition (F12),
  Find References (Shift+F12) and the document outline come from the same server.

## Finding the language server

The extension looks for `varg-lsp` in this order:

1. the path in the `varg.lsp.path` setting, if you set one,
2. beside `vargc` on your PATH — which is how a Varg release installs them,
3. `varg-lsp` anywhere on PATH.

If it finds none, it says so and leaves syntax highlighting on. It used to start a client against
a command that does not exist, announce that the server had started, and then do nothing.

Install Varg from the [releases page](https://github.com/LupusMalusDeviant/VARG/releases) and both
binaries end up in the same directory.

## Building it yourself

```bash
cd varg-vscode
npm install
npm run compile
npx vsce package        # produces varg-vscode-<version>.vsix
```

`code --install-extension varg-vscode-<version>.vsix` installs the result.
