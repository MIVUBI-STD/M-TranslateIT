import { runCommand } from "../shared/tauriBridge";

export type AppUpdateCheck = {
  configured: boolean;
  available: boolean;
  current_version: string;
  version: string | null;
  notes: string | null;
};

export type AppUpdateInstall = {
  installed: boolean;
  version: string | null;
  message: string;
};

let startupCheckStarted = false;

export const appUpdateApi = {
  async checkAtStartupOnce(): Promise<AppUpdateCheck | null> {
    if (startupCheckStarted) return null;
    startupCheckStarted = true;
    return runCommand<AppUpdateCheck>("check_app_update_once");
  },

  install(): Promise<AppUpdateInstall | null> {
    return runCommand<AppUpdateInstall>("install_app_update");
  },
};
