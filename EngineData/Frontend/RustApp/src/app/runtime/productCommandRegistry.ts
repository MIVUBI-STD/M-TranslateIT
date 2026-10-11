import type { AppRoute } from "../shared/types";

export type UtilityDestination =
  | "meeting-presets"
  | "meeting-transcript"
  | "settings-microphone"
  | "settings-captions"
  | "settings-spoken-terms"
  | "settings-preferred-words"
  | "settings-feedback"
  | "settings-diagnostics";

export type ProductCommand = {
  id: string;
  group: "Navigation" | "Meeting utilities" | "Text utilities" | "Settings & support";
  label: string;
  description: string;
  keywords: string[];
  disabled?: boolean;
  // Utility destinations move keyboard focus to the requested existing control.
  handsOffFocus?: boolean;
  run: () => void | Promise<void>;
};

export type ProductCommandContext = {
  meetingHasSession: boolean;
  meetingCanStart: boolean;
  meetingCanStop: boolean;
  quickTranslateReady: boolean;
  navigationLocked: boolean;
  navigate: (route: AppRoute) => void;
  openUtility: (destination: UtilityDestination) => void;
  toggleMeeting: () => void | Promise<void>;
  quickTranslateClipboard: () => void | Promise<void>;
};

// Search indexes existing product owners: no duplicate storage, translation path or new runtime commands.
export function buildProductCommands(context: ProductCommandContext): ProductCommand[] {
  const jump = (id: string, group: ProductCommand["group"], label: string, description: string,
    keywords: string[], destination: UtilityDestination): ProductCommand => ({
      id, group, label,
      description: context.navigationLocked ? "Stop My Voice recording before opening another tool." : description,
      keywords, handsOffFocus: true, disabled: context.navigationLocked ||
        (destination === "meeting-presets" && context.meetingHasSession),
      run: () => context.openUtility(destination),
    });
  return [
    { id: "navigate-meeting", group: "Navigation", label: "Open Meeting", description: "Go to live meeting translation.",
      keywords: ["call", "speech"], disabled: context.navigationLocked, run: () => context.navigate("meeting") },
    { id: "navigate-text", group: "Navigation", label: "Open Text", description: "Go to written translation.",
      keywords: ["translate", "written"], disabled: context.navigationLocked, run: () => context.navigate("text") },
    { id: "navigate-my-voice", group: "Navigation", label: "Open My Voice", description: "Choose or create your Meeting voice.",
      keywords: ["voice", "speaker"], disabled: context.navigationLocked, run: () => context.navigate("my-voice") },
    { id: "navigate-settings", group: "Navigation", label: "Open Settings", description: "Adjust Meeting and translation preferences.",
      keywords: ["preferences"], disabled: context.navigationLocked, run: () => context.navigate("settings") },
    { id: "meeting-toggle", group: "Meeting utilities",
      label: context.meetingHasSession ? "Stop Translation" : "Start Translation",
      description: context.meetingHasSession
        ? context.meetingCanStop ? "Stop the current Meeting safely." : "Meeting is changing state. Wait until Stop is available."
        : context.meetingCanStart ? "Start translation using the current setup." : "Unavailable until Meeting setup is ready.",
      // A 'stop' query must never start a Meeting, or vice versa.
      keywords: context.meetingHasSession ? ["stop", "end", "meeting", "translation"]
        : ["start", "begin", "meeting", "translation"],
      disabled: context.meetingHasSession ? !context.meetingCanStop : !context.meetingCanStart,
      run: context.toggleMeeting },
    jump("meeting-presets", "Meeting utilities", "Meeting Presets",
      context.meetingHasSession ? "Stop the current Meeting to manage presets." : "Save or use an existing Meeting setup.",
      ["saved", "devices", "configuration"], "meeting-presets"),
    jump("meeting-transcript", "Meeting utilities", "Export Meeting Transcript",
      "Open the Meeting transcript export controls. Requires a recorded session.",
      ["history", "export", "markdown", "txt", "notes"], "meeting-transcript"),
    { id: "quick-translate-clipboard", group: "Text utilities", label: "Translate Clipboard",
      description: context.quickTranslateReady ? "Translate copied text and show it in floating captions."
        : "Unavailable until Text translation is ready.",
      keywords: ["quick", "paste", "copy", "overlay", "caption"], disabled: !context.quickTranslateReady,
      run: context.quickTranslateClipboard },
    jump("floating-captions", "Text utilities", "Floating Caption Settings",
      "Show, resize, or change the appearance of translated captions.",
      ["subtitle", "overlay", "size", "contrast", "click through"], "settings-captions"),
    jump("meeting-microphone", "Settings & support", "Microphone & Mic Test",
      "Choose an input device, test the mic, or repair audio setup.",
      ["audio", "device", "sound", "repair"], "settings-microphone"),
    jump("spoken-terms", "Settings & support", "Manage Spoken Terms",
      "Help speech recognition identify names and uncommon words.",
      ["hotwords", "asr", "aliases", "vocabulary"], "settings-spoken-terms"),
    jump("preferred-words", "Settings & support", "Manage Preferred Translations",
      "Choose how specific Indonesian and English terms are translated.",
      ["terminology", "glossary", "csv"], "settings-preferred-words"),
    jump("translation-feedback", "Settings & support", "Translation Feedback",
      "Review and export locally reported translation issues.",
      ["report", "quality", "issues"], "settings-feedback"),
    jump("diagnostics", "Settings & support", "Open Diagnostics",
      "Troubleshoot the runtime and export redacted support details.",
      ["status", "performance", "watchdog", "help"], "settings-diagnostics"),
  ];
}
