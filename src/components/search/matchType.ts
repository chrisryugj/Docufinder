import type { SearchResultMatchType } from "../../types/search";

export function getMatchTypeBadge(matchType: SearchResultMatchType): {
  label: string;
  variant: "keyword" | "semantic" | "hybrid";
} {
  switch (matchType) {
    case "semantic":
      return { label: "🧠 의미", variant: "semantic" };
    case "hybrid":
      return { label: "하이브리드", variant: "hybrid" };
    default:
      return { label: "키워드", variant: "keyword" };
  }
}
