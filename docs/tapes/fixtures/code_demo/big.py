from __future__ import annotations


class Greeter:
    def __init__(self, prefix: str = "Hello"):
        self.prefix = prefix

    def greet(self, name: str) -> str:
        return f"{self.prefix}, {name}"


def parse_numbers(text: str) -> list[int]:
    out: list[int] = []
    for raw in text.split(","):
        raw = raw.strip()
        if not raw:
            continue
        out.append(int(raw))
    return out


def compute_summary(values: list[int]) -> dict[str, int]:
    total = 0
    even = 0
    odd = 0
    for v in values:
        total += v
        if v % 2 == 0:
            even += 1
        else:
            odd += 1

    # A longer block that should mostly be omitted under tight budgets.
    histogram: dict[int, int] = {}
    for v in values:
        bucket = (v // 10) * 10
        histogram[bucket] = histogram.get(bucket, 0) + 1

    top_bucket = max(histogram, key=histogram.get, default=0)
    return {"total": total, "even": even, "odd": odd, "top_bucket": top_bucket}


def main() -> None:
    greeter = Greeter()
    print(greeter.greet("world"))

    values = parse_numbers("1,2,3,4,5,6,7,8,9,10,21,22,23,24,25,26,27,28,29,30")
    summary = compute_summary(values)
    print("summary:", summary)


if __name__ == "__main__":
    main()
