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

let startupCheckPromise: Promise<AppUpdateCheck | null> | null = null;

export const appUpdateApi = {
  checkAtStartupOnce(): Promise<AppUpdateCheck | null> {
    if (!startupCheckPromise) {
      startupCheckPromise = runCommand<AppUpdateCheck>("check_app_update_once");
    }
    return startupCheckPromise;
  },

  install(): Promise<AppUpdateInstall | null> {
    return runCommand<AppUpdateInstall>("install_app_update");
  },
};
