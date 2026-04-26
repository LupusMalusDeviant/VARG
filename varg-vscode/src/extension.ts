import * as path from 'path';
import * as vscode from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind,
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext) {
    // Determine the path to the varg-lsp binary
    const config = vscode.workspace.getConfiguration('varg.lsp');
    let serverPath = config.get<string>('path', '');

    if (!serverPath) {
        // Default: look for varg-lsp in PATH, or use a well-known location
        serverPath = 'varg-lsp';
    }

    const serverOptions: ServerOptions = {
        command: serverPath,
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

    // Start the client (and the server)
    client.start();

    context.subscriptions.push({
        dispose: () => {
            if (client) {
                client.stop();
            }
        },
    });

    vscode.window.showInformationMessage('Varg Language Server started');
}

export function deactivate(): Thenable<void> | undefined {
    if (client) {
        return client.stop();
    }
    return undefined;
}
