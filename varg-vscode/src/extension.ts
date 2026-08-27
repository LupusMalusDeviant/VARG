import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind,
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

const EXE = process.platform === 'win32' ? '.exe' : '';

/**
 * Find the language server.
 *
 * It used to be `varg-lsp` and nothing else: if the binary was not on PATH the extension started
 * a client against a command that does not exist, reported that the server had started, and then
 * did nothing — no diagnostics, no hover, no explanation. A release ships `varg-lsp` beside
 * `vargc`, so looking there costs nothing and is where it usually is.
 */
function findServer(): { command: string; how: string } | undefined {
    const configured = vscode.workspace
        .getConfiguration('varg.lsp')
        .get<string>('path', '')
        .trim();
    if (configured) {
        return fs.existsSync(configured)
            ? { command: configured, how: 'varg.lsp.path' }
            : undefined;
    }

    // Beside vargc, wherever that is: an installation keeps them together.
    for (const dir of (process.env.PATH ?? '').split(path.delimiter)) {
        if (!dir) {
            continue;
        }
        if (fs.existsSync(path.join(dir, `vargc${EXE}`))) {
            const beside = path.join(dir, `varg-lsp${EXE}`);
            if (fs.existsSync(beside)) {
                return { command: beside, how: 'beside vargc' };
            }
        }
        const onPath = path.join(dir, `varg-lsp${EXE}`);
        if (fs.existsSync(onPath)) {
            return { command: onPath, how: 'PATH' };
        }
    }
    return undefined;
}

export function activate(context: vscode.ExtensionContext) {
    const server = findServer();
    if (!server) {
        // Syntax highlighting is contributed declaratively and works regardless; only the
        // language server is missing, and saying which is more use than a generic failure.
        vscode.window.showWarningMessage(
            'Varg: no varg-lsp found, so diagnostics and hover are off. ' +
                'Install Varg so that varg-lsp sits beside vargc, or set varg.lsp.path.'
        );
        return;
    }

    const serverOptions: ServerOptions = {
        command: server.command,
        transport: TransportKind.stdio,
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'varg' }],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*.varg'),
        },
    };

    client = new LanguageClient(
        'varg-lsp',
        'Varg Language Server',
        serverOptions,
        clientOptions
    );

    client.start();

    // In the status bar rather than a popup: it is worth being able to check which server is
    // running, and not worth interrupting anyone to say so on every window.
    const status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
    status.text = 'Varg';
    status.tooltip = `varg-lsp (${server.how})\n${server.command}`;
    status.show();

    context.subscriptions.push(status, {
        dispose: () => {
            if (client) {
                client.stop();
            }
        },
    });
}

export function deactivate(): Thenable<void> | undefined {
    if (client) {
        return client.stop();
    }
    return undefined;
}
