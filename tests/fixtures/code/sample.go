package main

import "fmt"

func greet(name string) {
    fmt.Printf("Hello, %s\n", name)
}

func compute(n int) int {
    total := 0
    for i := 0; i < n; i++ {
        if i%2 == 0 {
            total += i
        } else {
            total += i * 2
        }
    }
    return total
}

func main() {
    greet("world")

    for i := 0; i < 3; i++ {
        fmt.Println("i:", i)
    }

    value := compute(10)
    fmt.Println("value:", value)
}

