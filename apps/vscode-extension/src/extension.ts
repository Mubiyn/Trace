import * as vscode from "vscode";
import { getWebviewHtml } from "./webviewHtml";

let panel: vscode.WebviewPanel | undefined;

export function activate(context: vscode.ExtensionContext) {
  const openPanel = vscode.commands.registerCommand("graph.openPanel", () => {
    const folder = vscode.workspace.workspaceFolders?.[0];
    if (!folder) {
      void vscode.window.showWarningMessage("Open a workspace folder to use Graph.");
      return;
    }

    if (panel) {
      panel.reveal(vscode.ViewColumn.One);
      return;
    }

    panel = vscode.window.createWebviewPanel(
      "graphPanel",
      "Graph",
      vscode.ViewColumn.One,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [vscode.Uri.joinPath(context.extensionUri, "media")],
      },
    );

    panel.webview.html = getWebviewHtml(
      panel.webview,
      context.extensionUri,
      folder.uri.fsPath,
    );
    panel.webview.onDidReceiveMessage(async (message: { type?: string; path?: string; line?: number }) => {
      if (message.type !== "openFile" || !message.path) {
        return;
      }
      const target = vscode.Uri.joinPath(folder.uri, message.path);
      const doc = await vscode.workspace.openTextDocument(target);
      const line = Math.max(0, (message.line ?? 1) - 1);
      const editor = await vscode.window.showTextDocument(doc, vscode.ViewColumn.Beside);
      const position = new vscode.Position(line, 0);
      editor.selection = new vscode.Selection(position, position);
      editor.revealRange(new vscode.Range(position, position));
    });

    panel.onDidDispose(() => {
      panel = undefined;
    });
  });

  context.subscriptions.push(openPanel);
}

export function deactivate() {
  panel?.dispose();
}
