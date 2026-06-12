"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const vscode = __importStar(require("vscode"));
const webviewHtml_1 = require("./webviewHtml");
let panel;
function activate(context) {
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
        panel = vscode.window.createWebviewPanel("graphPanel", "Graph", vscode.ViewColumn.One, {
            enableScripts: true,
            retainContextWhenHidden: true,
            localResourceRoots: [vscode.Uri.joinPath(context.extensionUri, "media")],
        });
        panel.webview.html = (0, webviewHtml_1.getWebviewHtml)(panel.webview, context.extensionUri, folder.uri.fsPath);
        panel.webview.onDidReceiveMessage(async (message) => {
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
function deactivate() {
    panel?.dispose();
}
//# sourceMappingURL=extension.js.map