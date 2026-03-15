"""Equivalent benchmark to benchmark_realistic.varg in Python"""

def fibonacci(n: int) -> int:
    fa, fb = 0, 1
    for _ in range(n):
        fa, fb = fb, fa + fb
    return fa

def sum_range(start_val: int, end_val: int) -> int:
    total = 0
    idx = start_val
    while idx < end_val:
        total += idx
        idx += 1
    return total

class Benchmark:
    def __init__(self):
        self.counter = 0

    def run_once(self):
        # 1. Fibonacci(35)
        fib = fibonacci(35)
        self.counter += fib

        # 2. String building (10000 concats)
        result = ""
        for _ in range(10000):
            result += "x"

        # 3. Array fill + sum (10000 elements)
        numbers = [0]
        for i in range(1, 10000):
            numbers.append(i)
        total = sum(numbers)

        # 4. Nested loop (matrix-like 200x200)
        matrix_sum = 0
        for row in range(200):
            for col in range(200):
                matrix_sum += row * col

        # 5. Sum range
        range_sum = sum_range(0, 1000)
        self.counter += range_sum

    def run(self):
        print("=== Python Benchmark (100 iterations) ===")
        for _ in range(100):
            self.run_once()
        print(self.counter)
        print("=== Done ===")

if __name__ == "__main__":
    Benchmark().run()
