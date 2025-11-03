def greet(name: str):
    print(f"Hello, {name}")


def compute(n: int) -> int:
    total = 0
    for i in range(n):
        if i % 2 == 0:
            total += i
        else:
            total += i * 2
    return total


def main():
    greet("world")
    for i in range(3):
        print("i:", i)
    value = compute(10)
    print("value:", value)


if __name__ == "__main__":
    main()
