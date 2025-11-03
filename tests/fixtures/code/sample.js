function greet(name) {
  console.log(`Hello, ${name}`);
}

function compute(n) {
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

function main() {
  greet("world");

  for (let i = 0; i < 3; i++) {
    console.log("i:", i);
  }

  const value = compute(10);
  console.log("value:", value);
}

main();

