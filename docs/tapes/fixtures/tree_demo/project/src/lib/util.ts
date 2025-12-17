export function greet(name: string) {
  console.log(`Hello, ${name}`);
}

export function compute(n: number): number {
  let total = 0;
  for (let i = 0; i < n; i++) {
    if (i % 2 === 0) {
      total += i;
    } else {
      total += i * 2;
    }
  }
  return total;
}
