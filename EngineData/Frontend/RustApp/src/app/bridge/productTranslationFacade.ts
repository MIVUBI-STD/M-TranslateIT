import { runtimeApi } from "./runtimeApi";
import { errorMessage } from "../shared/state";

export type ProductTranslationResult = {
  ok: boolean;
  source: string;
  translated: string;
  status: string;
  message: string;
  blocker: string;
  needsReview: boolean;
  reviewHints: string[];
};

export async function runProductTranslation(source: string): Promise<ProductTranslationResult> {
  const cleaned = source.trim();
  if (!cleaned) {
    return {
      ok: false,
      source,
      translated: "",
      status: "empty",
      message: "Type or paste something to translate.",
      blocker: "text_translation:empty_input",
      needsReview: false,
      reviewHints: [],
    };
  }
  try {
    const result = await runtimeApi.translateText(cleaned);
    return {
      ok: Boolean(result.ok),
      source: cleaned,
      translated: result.translated_text,
      status: result.state,
      message: result.user_message,
      blocker: result.blocker,
      needsReview: Boolean(result.needs_review),
      reviewHints: Array.isArray(result.review_hints) ? result.review_hints : [],
    };
  } catch (error) {
    return {
      ok: false,
      source: cleaned,
      translated: "",
      status: "frontend_bridge_error",
      message: "Translation is unavailable right now. Try again in a moment.",
      blocker: errorMessage(error),
      needsReview: false,
      reviewHints: [],
    };
  }
}
