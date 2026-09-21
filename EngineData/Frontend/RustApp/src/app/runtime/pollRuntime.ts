export function startRuntimePoll(
  callback: () => void | Promise<void>,
  intervalMs: number,
): () => void {
  void callback();
  const timer = window.setInterval(() => void callback(), intervalMs);
  return () => window.clearInterval(timer);
}
