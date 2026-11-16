import React from "react";
import { createRoot } from "react-dom/client";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";

mockWindows("main");
mockIPC((command, args) => window.qualificationInvoke(command, args), { shouldMockEvents: true });
const { default: App } = await import("../../../src/App");
createRoot(document.getElementById("root")).render(<App />);
