import type { CloseDialogAction, CloseVerdict } from "./closePolicy";
import {
  destroyTranslateItWindows,
  resolveNativeCloseVerdict,
  stopAndResolveNativeClose,
} from "./nativeCloseRuntime";
import { compact } from "../shared/state";

export type CloseControllerState = {
  open: boolean;
  title: string;
  message: string;
  action: CloseDialogAction;
  busy: boolean;
  checkInFlight: boolean;
  closeAfterExistingStop: boolean;
};

export function createCloseController(): {
  read: () => CloseControllerState;
  keepOpen: () => CloseControllerState;
  inspect: () => Promise<CloseControllerState>;
  primary: () => Promise<CloseControllerState>;
  closeWindow: () => Promise<CloseControllerState>;
} {
  let state: CloseControllerState = {
    open: false,
    title: "Close TranslateIT?",
    message: "",
    action: null,
    busy: false,
    checkInFlight: false,
    closeAfterExistingStop: false,
  };

  const read = (): CloseControllerState => ({ ...state });

  const showDialog = (
    title: string,
    message: string,
    action: CloseDialogAction,
  ) => {
    state = {
      ...state,
      title,
      message: compact(message, "Status unavailable.", 220),
      action,
      open: true,
    };
  };

  const applyVerdict = async (verdict: CloseVerdict) => {
    if (verdict.kind === "destroy") {
      await destroyTranslateItWindows();
      state = { ...state, open: false, closeAfterExistingStop: false };
      return;
    }
    if (verdict.kind === "stop-and-close") {
      showDialog(
        "Translation is still running",
        "Stop & Close ends Meeting translation safely before closing TranslateIT.",
        "stop",
      );
      return;
    }
    if (verdict.kind === "wait-for-stop") {
      state = { ...state, closeAfterExistingStop: true };
      showDialog(verdict.title, verdict.message, null);
      return;
    }
    showDialog(verdict.title, verdict.message, verdict.action);
  };

  const inspect = async (): Promise<CloseControllerState> => {
    if (state.checkInFlight || state.busy) return read();
    state = { ...state, checkInFlight: true };
    try {
      await applyVerdict(await resolveNativeCloseVerdict());
    } catch {
      showDialog(
        "Couldn't close TranslateIT",
        "TranslateIT couldn't confirm that it is safe to close. Keep the app open and try again.",
        "retry",
      );
    } finally {
      state = { ...state, checkInFlight: false };
    }
    return read();
  };

  return {
    read,

    keepOpen(): CloseControllerState {
      state = {
        ...state,
        open: false,
        action: null,
        busy: false,
        closeAfterExistingStop: false,
      };
      return read();
    },

    inspect,

    async primary(): Promise<CloseControllerState> {
      if (state.action === "retry") {
        state = { ...state, open: false };
        return inspect();
      }
      if (state.action !== "stop" || state.busy) return read();

      state = { ...state, busy: true };
      try {
        await applyVerdict(await stopAndResolveNativeClose());
      } catch {
        showDialog(
          "Couldn't close TranslateIT",
          "The app will stay open. Try again in a moment.",
          "retry",
        );
      } finally {
        state = { ...state, busy: false };
      }
      return read();
    },

    async closeWindow(): Promise<CloseControllerState> {
      state = { ...state, closeAfterExistingStop: false };
      try {
        await destroyTranslateItWindows();
        state = { ...state, open: false };
      } catch {
        showDialog(
          "Couldn't close TranslateIT",
          "The floating caption couldn't close safely. TranslateIT will stay open.",
          "retry",
        );
      }
      return read();
    },
  };
}
