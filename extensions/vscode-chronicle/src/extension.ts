import * as net from 'net';
import { randomUUID } from 'crypto';
import * as vscode from 'vscode';

const DEFAULT_SOCKET = '/tmp/chronicle.sock';

function emit(socketPath: string, event: Record<string, unknown>): Promise<void> {
  const payload = JSON.stringify({ type: 'emit_event', event });
  const body = Buffer.from(payload, 'utf8');
  const frame = Buffer.alloc(4);
  frame.writeUInt32BE(body.length, 0);

  return new Promise((resolve, reject) => {
    const sock = net.createConnection(socketPath);
    sock.once('error', reject);
    sock.once('connect', () => sock.write(Buffer.concat([frame, body])));
    sock.once('data', () => {
      sock.end();
      resolve();
    });
  });
}

function baseEvent(type: string, metadata: Record<string, unknown> = {}) {
  const folder = vscode.workspace.workspaceFolders?.[0];
  return {
    version: '1.0',
    id: randomUUID(),
    timestamp: Date.now(),
    source: 'vscode',
    category: 'ide',
    type,
    project: folder?.name ?? null,
    workspace: folder?.uri.fsPath ?? null,
    duration_ms: null,
    metadata,
  };
}

export function activate(context: vscode.ExtensionContext) {
  const socket =
    vscode.workspace.getConfiguration('chronicle').get<string>('socket') ??
    DEFAULT_SOCKET;

  const send = (type: string, metadata: Record<string, unknown> = {}) => {
    emit(socket, baseEvent(type, metadata)).catch(() => {});
  };

  context.subscriptions.push(
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      if (!editor) return;
      send('ide.file.focus', { file: editor.document.fileName });
    }),
    vscode.workspace.onDidSaveTextDocument((doc) => {
      send('ide.file.saved', { file: doc.fileName });
    }),
    vscode.commands.registerCommand('chronicle.emitTestRun', () => {
      send('ide.test.run', { action: 'manual' });
    }),
    vscode.commands.registerCommand('chronicle.emitDebugStart', () => {
      send('ide.debug.start', { action: 'manual' });
    }),
  );
}

export function deactivate() {}
