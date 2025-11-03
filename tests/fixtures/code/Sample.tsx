import React from 'react';

type Props = { name: string };

function Item({ i }: { i: number }) {
  return <li>i: {i}</li>;
}

export default function Sample({ name }: Props) {
  function compute(n: number): number {
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

  const value = compute(10);

  return (
    <div>
      <h1>Hello, {name}</h1>
      <ul>
        {[0, 1, 2].map((i) => (
          <Item key={i} i={i} />
        ))}
      </ul>
      <p>value: {value}</p>
    </div>
  );
}

