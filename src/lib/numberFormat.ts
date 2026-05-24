const TOKEN_UNITS = [
  { value: 1_000_000_000, suffix: "B" },
  { value: 1_000_000, suffix: "M" },
  { value: 1_000, suffix: "K" },
] as const;

export function formatTokenCount(value: number | null | undefined): string {
  const numericValue = value ?? 0;
  const abs = Math.abs(numericValue);

  for (const unit of TOKEN_UNITS) {
    if (abs >= unit.value) {
      const scaled = numericValue / unit.value;
      const digits = Math.abs(scaled) >= 10 ? 0 : 1;
      return `${scaled.toFixed(digits).replace(/\.0$/, "")}${unit.suffix}`;
    }
  }

  return String(numericValue);
}

export function formatTokenCountFixed(value: number | null | undefined): string {
  const numericValue = value ?? 0;
  const abs = Math.abs(numericValue);

  for (const unit of TOKEN_UNITS) {
    if (abs >= unit.value) {
      return `${(numericValue / unit.value).toFixed(1)}${unit.suffix.toLowerCase()}`;
    }
  }

  return String(numericValue);
}
