import type { AppRoute } from "../shared/types";

export type UtilityDestination = "settings-microphone" | "settings-captions" | "settings-diagnostics";

export type ProductCommand = {
  id: string;
  group: "Navigation" | "Meeting utilities" | "Settings & support";
  label: string;
  description: string;
  keywords: string[];
  disabled?: boolean;
  handsOffFocus?: boolean;
  run: () => void | Promise<void>;
};

export type ProductCommandContext = {
  meetingCanStart: boolean;
  meetingCanStop: boolean;
  meetingActionLabel: string;
  navigationLocked: boolean;
  navigate: (route: AppRoute) => void;
  openUtility: (destination: UtilityDestination) => void;
  toggleMeeting: () => void | Promise<void>;
};

export function buildProductCommands(context: ProductCommandContext): ProductCommand[] {
  const jump = (id: string, label: string, destination: UtilityDestination,
    keywords: string[]): ProductCommand => ({
    id, label, keywords, group: "Settings & support", handsOffFocus: true,
    description: context.navigationLocked ? "Stop My Voice recording first." : "Open the setting.",
    disabled: context.navigationLocked,
    run: () => context.openUtility(destination),
  });
  return [
    { id: "navigate-meeting", group: "Navigation", label: "Meeting", description: "Open Meeting.",
      keywords: ["call", "speech"], disabled: context.navigationLocked, run: () => context.navigate("meeting") },
    { id: "navigate-text", group: "Navigation", label: "Text", description: "Open Text translation.",
      keywords: ["translate", "written"], disabled: context.navigationLocked, run: () => context.navigate("text") },
    { id: "navigate-my-voice", group: "Navigation", label: "My Voice", description: "Choose your Meeting voice.",
      keywords: ["voice", "speaker"], disabled: context.navigationLocked, run: () => context.navigate("my-voice") },
    { id: "navigate-settings", group: "Navigation", label: "Settings", description: "Open Settings.",
      keywords: ["preferences"], disabled: context.navigationLocked, run: () => context.navigate("settings") },
    { id: "meeting-toggle", group: "Meeting utilities", label: context.meetingActionLabel,
      description: context.meetingCanStop ? "Stop Meeting translation." : "Start Meeting translation.",
      keywords: context.meetingCanStop ? ["stop", "end", "meeting", "translation"]
        : context.meetingCanStart ? ["start", "begin", "meeting", "translation"] : ["meeting", "status"],
      disabled: !context.meetingCanStop && !context.meetingCanStart, run: context.toggleMeeting },
    jump("meeting-microphone", "Voice & Audio", "settings-microphone", ["microphone", "input", "output"]),
    jump("meeting-captions", "Meeting Captions", "settings-captions", ["captions", "subtitles"]),
    jump("diagnostics", "Help", "settings-diagnostics", ["help", "diagnostics", "repair"]),
  ];
}
