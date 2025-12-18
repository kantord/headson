type Summary = {
  total: number;
  even: number;
  odd: number;
  topBucket: number;
};

export function parseNumbers(text: string): number[] {
  const out: number[] = [];
  for (const raw of text.split(",")) {
    const s = raw.trim();
    if (!s) continue;
    out.push(Number.parseInt(s, 10));
  }
  return out;
}

export function computeSummary(values: number[]): Summary {
  let total = 0;
  let even = 0;
  let odd = 0;
  for (const v of values) {
    total += v;
    if (v % 2 === 0) even++;
    else odd++;
  }

  // A longer block that should mostly be omitted under tight budgets.
  const histogram = new Map<number, number>();
  for (const v of values) {
    const bucket = Math.floor(v / 10) * 10;
    histogram.set(bucket, (histogram.get(bucket) ?? 0) + 1);
  }

  let topBucket = 0;
  let topCount = -1;
  for (const [bucket, count] of histogram.entries()) {
    if (count > topCount) {
      topCount = count;
      topBucket = bucket;
    }
  }

  return { total, even, odd, topBucket };
}

export function main(): void {
  const values = parseNumbers(
    "1,2,3,4,5,6,7,8,9,10,21,22,23,24,25,26,27,28,29,30",
  );
  const summary = computeSummary(values);
  console.log("summary:", summary);
}

main();
