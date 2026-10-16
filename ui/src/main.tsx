import React from "react";
import { createRoot } from "react-dom/client";
import "./index.css";
import "@/i18n";
import { ThemeProvider } from "@/hooks/theme";
import { LocaleProvider } from "@/hooks/locale";
import { App } from "./App";

createRoot(document.getElementById("root")!).render(
  <React.StrictMode><ThemeProvider><LocaleProvider><App /></LocaleProvider></ThemeProvider></React.StrictMode>,
);
