"""Heavy benchmark equivalent"""

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

class HeavyBench:
    def __init__(self):
        self.counter = 0

    def run_once(self):
        fib = fibonacci(40)
        self.counter += fib

        matrix_sum = 0
        for row in range(300):
            for col in range(300):
                matrix_sum += row * col

        range_sum = sum_range(0, 50000)
        self.counter += range_sum

    def run(self):
        print("=== Python Heavy Benchmark (1000 iter) ===")
        for _ in range(1000):
            self.run_once()
        print(self.counter)
        print("=== Done ===")

if __name__ == "__main__":
    HeavyBench().run()
