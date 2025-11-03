public class Sample {
    static void greet(String name) {
        System.out.println("Hello, " + name);
    }

    static int compute(int n) {
        int total = 0;
        for (int i = 0; i < n; i++) {
            if (i % 2 == 0) {
                total += i;
            } else {
                total += i * 2;
            }
        }
        return total;
    }

    public static void main(String[] args) {
        greet("world");

        for (int i = 0; i < 3; i++) {
            System.out.println("i: " + i);
        }

        int value = compute(10);
        System.out.println("value: " + value);
    }
}

