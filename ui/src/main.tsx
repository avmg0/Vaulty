import React from "react";
import { createRoot } from "react-dom/client";
import "./index.css";
import { ThemeProvider } from "@/hooks/theme";
import { App } from "./App";

createRoot(document.getElementById("root")!).render(
  <React.StrictMode><ThemeProvider><App /></ThemeProvider></React.StrictMode>,
);
