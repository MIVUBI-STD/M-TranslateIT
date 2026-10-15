import "./styles/app.css";
import { mount } from "svelte";
import App from "./App.svelte";
import TranslationOverlay from "./pages/TranslationOverlay.svelte";
import QuickAccess from "./pages/QuickAccess.svelte";

const target = document.querySelector<HTMLDivElement>("#app");
if (!target) throw new Error("TranslateIT app root was not found.");

const view = new URLSearchParams(window.location.search).get("view");
if (view === "translation-overlay") document.documentElement.classList.add("ti-overlay-document");
if (view === "quick-access") document.documentElement.classList.add("ti-quick-access-document");

mount(view === "translation-overlay" ? TranslationOverlay : view === "quick-access" ? QuickAccess : App, { target });
