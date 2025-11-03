#include <iostream>

void greet(const std::string& name) {
    std::cout << "Hello, " << name << "!" << std::endl;
}

int main() {
    greet("world");

    for (int i = 0; i < 5; ++i) {
        if (i % 2 == 0) {
            std::cout << "even: " << i << std::endl;
        } else {
            std::cout << "odd: " << i << std::endl;
        }
    }

    return 0;
}

