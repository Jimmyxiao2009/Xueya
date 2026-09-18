import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "@/App";
import { startForegroundSync } from "@/stores/cloudStore";
import "@/styles/index.css";

startForegroundSync();

const container = document.getElementById("root");
if (!container) throw new Error("#root is missing from index.html");

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
