import "./styles/app.css";
import { mount } from "svelte";
import App from "./App.svelte";
import TranslationOverlay from "./pages/TranslationOverlay.svelte";

const target = document.querySelector<HTMLDivElement>("#app");
if (!target) throw new Error("TranslateIT app root was not found.");

const overlayView = new URLSearchParams(window.location.search).get("view") === "translation-overlay";
if (overlayView) document.documentElement.classList.add("ti-overlay-document");

mount(overlayView ? TranslationOverlay : App, { target });
