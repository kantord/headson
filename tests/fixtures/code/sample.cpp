// A small, representative C++ source file demonstrating common constructs.

#include <iostream>
#include <vector>
#include <string>
#include <optional>
#include <algorithm>

namespace demo {

template <typename T>
class Box {
public:
  explicit Box(T v) : value_(std::move(v)) {}
  const T& get() const { return value_; }
  void set(T v) { value_ = std::move(v); }
private:
  T value_;
};

struct User {
  int id;
  std::string name;
  std::vector<std::string> roles;
};

std::optional<User> find_user(const std::vector<User>& users, int id) {
  for (const auto& u : users) {
    if (u.id == id) return u;
  }
  return std::nullopt;
}

void print_users(const std::vector<User>& users) {
  std::cout << "Users (" << users.size() << "):\n";
  for (const auto& u : users) {
    std::cout << " - " << u.id << ": " << u.name << "\n";
    if (!u.roles.empty()) {
      std::cout << "   roles:";
      for (const auto& r : u.roles) std::cout << ' ' << r;
      std::cout << "\n";
    }
  }
}

int sum(const std::vector<int>& xs) {
  int acc = 0;
  for (int x : xs) acc += x;
  return acc;
}

} // namespace demo

int main() {
  using namespace demo;

  std::vector<User> users = {
    {1, "Ana", {"admin", "dev"}},
    {2, "Bo", {"analyst"}},
    {3, "Cy", {}}
  };

  print_users(users);

  auto u = find_user(users, 2);
  if (u) {
    Box<std::string> b(u->name);
    b.set(b.get() + "!");
    std::cout << "Found: " << b.get() << "\n";
  }

  std::vector<int> xs{1,2,3,4,5};
  std::cout << "sum(xs) = " << sum(xs) << "\n";

  // A lambda and algorithm usage
  std::vector<int> evens;
  std::copy_if(xs.begin(), xs.end(), std::back_inserter(evens), [](int v){
    return v % 2 == 0;
  });
  std::cout << "evens:";
  for (int v : evens) std::cout << ' ' << v;
  std::cout << "\n";

  return 0;
}

