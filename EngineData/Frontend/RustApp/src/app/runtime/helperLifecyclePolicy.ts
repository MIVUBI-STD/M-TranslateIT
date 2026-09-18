export function shouldAutoStartHelper(state: string): boolean {
  return state === "not_started";
}

export function shouldExplicitlyRestartHelper(state: string): boolean {
  return state === "not_started" || state === "stopped";
}
