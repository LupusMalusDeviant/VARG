// Same logic but with idiomatic Rust (what Varg SHOULD generate)
use std::time::Instant;

struct Benchmark {}

impl Benchmark {
    pub fn run_once(&mut self) {
        // 1. Fibonacci(35)
        let n = 35;
        let mut fa: i64 = 0;
        let mut fb: i64 = 1;
        for _ in 0..n {
            let temp = fb;
            fb = fa + fb;
            fa = temp;
        }

        // 2. String building (10000 concats) — push_str without double alloc
        let mut result = String::with_capacity(10000);
        for _ in 0..10000 {
            result.push('x');
        }

        // 3. Array fill + sum (10000 elements)
        let mut numbers: Vec<i64> = Vec::with_capacity(10000);
        numbers.push(0);
        for i in 1..10000i64 {
            numbers.push(i);
        }
        let sum: i64 = numbers.iter().sum();

        // 4. Nested loop (matrix multiply style)
        let mut matrix_sum: i64 = 0;
        for row in 0..200i64 {
            for col in 0..200i64 {
                matrix_sum += row * col;
            }
        }
    }

    pub fn run(&mut self) {
        let start = Instant::now();
        for _ in 0..100 {
            self.run_once();
        }
        let elapsed = start.elapsed();
        println!("Rust (optimized): {:.2}ms", elapsed.as_secs_f64() * 1000.0);
    }
}

fn main() {
    let mut bench = Benchmark {};
    bench.run();
}
