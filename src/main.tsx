import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { attachConsole } from "@tauri-apps/plugin-log";
import App from "./App";
import QuickPanel from "./components/QuickPanel";
import "./App.css";

// Mirror backend logs into the webview devtools console.
attachConsole();

// One frontend bundle serves both windows; the label decides what to render.
const isQuickWindow = getCurrentWebviewWindow().label === "quick";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {isQuickWindow ? <QuickPanel /> : <App />}
  </React.StrictMode>,
);
