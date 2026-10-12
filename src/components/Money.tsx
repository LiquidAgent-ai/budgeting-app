import { money } from "@/lib/format";

export function Money({
  value,
  accent,
  className = "",
}: {
  value: number;
  accent?: "good" | "danger" | "muted";
  className?: string;
}) {
  const color =
    accent === "good"
      ? "text-good"
      : accent === "danger"
        ? "text-danger"
        : accent === "muted"
          ? "text-ink-soft/70"
          : "";
  return <span className={`tabular-nums ${color} ${className}`}>{money(value)}</span>;
}
