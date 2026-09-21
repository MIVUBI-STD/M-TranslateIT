import type { AppRoute } from "../shared/types";

export type ProductCommand = {
  id: string;
  label: string;
  description: string;
  keywords: string[];
  shortcut?: string;
  disabled?: boolean;
  run: () => void | Promise<void>;
};

export type ProductCommandContext = {
  route: AppRoute;
  meetingHasSession: boolean;
  meetingCanStart: boolean;
  meetingCanStop: boolean;
  quickTranslateReady: boolean;
  navigate: (route: AppRoute) => void;
  toggleMeeting: () => void | Promise<void>;
  quickTranslateClipboard: () => void | Promise<void>;
};

export function buildProductCommands(context: ProductCommandContext): ProductCommand[] {
  return [
    {
      id: "navigate-meeting",
      label: "Open Meeting",
      description: "Go to live meeting translation.",
      keywords: ["meeting", "call", "voice"],
      run: () => context.navigate("meeting"),
    },
    {
      id: "navigate-text",
      label: "Open Text",
      description: "Go to Indonesian ↔ English text translation.",
      keywords: ["text", "translate", "written"],
      run: () => context.navigate("text"),
    },
    {
      id: "navigate-my-voice",
      label: "Open My Voice",
      description: "Choose or manage the voice other people hear.",
      keywords: ["voice", "speaker", "tts"],
      run: () => context.navigate("my-voice"),
    },
    {
      id: "navigate-settings",
      label: "Open Settings",
      description: "Open meeting, translation, and diagnostic settings.",
      keywords: ["settings", "audio", "diagnostics"],
      run: () => context.navigate("settings"),
    },
    {
      id: "quick-translate-clipboard",
      label: "Quick Translate Clipboard",
      description: "Translate clipboard text and show it in the floating caption.",
      keywords: ["quick", "clipboard", "overlay", "caption"],
      disabled: !context.quickTranslateReady,
      run: context.quickTranslateClipboard,
    },
    {
      id: "meeting-toggle",
      label: context.meetingCanStop ? "Stop Translation" : "Start Translation",
      description: context.meetingCanStop
        ? "Stop the current Meeting translation safely."
        : "Start Meeting translation using the current setup.",
      keywords: ["start", "stop", "meeting", "translation"],
      disabled: context.meetingHasSession ? !context.meetingCanStop : !context.meetingCanStart,
      run: context.toggleMeeting,
    },
  ];
}
