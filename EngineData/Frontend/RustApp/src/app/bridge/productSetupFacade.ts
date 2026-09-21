import { applicationRuntimeApi } from "./applicationRuntimeApi";
import { runtimeApi } from "./runtimeApi";
import { compact } from "../shared/state";

export type ProductSetupAction = "check-readiness" | "verify-models";
export type ProductRecoveryAction = "fix-setup";

export async function runProductSetupAction(action: ProductSetupAction): Promise<string> {
  if (action === "check-readiness") {
    const result = await runtimeApi.verifyRequiredOutboundAiReadiness().catch(() => null);
    return result?.ok
      ? "TranslateIT is ready."
      : "TranslateIT still needs attention. Try Check Again, then open Help if the problem continues.";
  }

  const result = await runtimeApi.verifyModels().catch(() => null);
  const blockers = Array.isArray(result?.blockers) ? result.blockers.join("; ") : "";
  return compact(
    result?.note ?? blockers,
    result?.ok ? "All required AI files are available." : "Some required AI files still need attention.",
  );
}

export async function runProductRecoveryAction(action: ProductRecoveryAction): Promise<string> {
  if (action !== "fix-setup") return "No product recovery action was selected.";

  const result = await applicationRuntimeApi.dispatchIntent("fix_setup");
  if (!result.ok) {
    return compact(
      result.message,
      "Setup still needs attention. Try Check Again, then open Help if needed.",
    );
  }

  const hasBlockingProblem = result.snapshot.problems.some((problem) => problem.severity === "blocking");
  if (hasBlockingProblem) {
    return "Setup still needs attention. Try Check Again, then open Help if needed.";
  }

  return "TranslateIT is ready. Check Meeting again; the TranslateIT microphone may still need attention.";
}
