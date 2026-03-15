// Varg-generated code (manually adapted as standalone for benchmarking)
use std::time::Instant;

struct Benchmark {}

impl Benchmark {
    pub fn run_once(&mut self) {
        // 1. Fibonacci(35)
        let mut n = 35;
        let mut fa: i64 = 0;
        let mut fb: i64 = 1;
        let mut fi = 0;
        while fi < n {
            let temp = fb;
            fb = fa + fb;
            fa = temp;
            fi = fi + 1;
        }

        // 2. String building (10000 concats)
        let mut result = "".to_string();
        let mut si = 0;
        while si < 10000 {
            result.push_str(&("x".to_string()).to_string());
            si = si + 1;
        }

        // 3. Array fill + sum (10000 elements)
        let mut numbers: Vec<i64> = vec![0];
        let mut ai: i64 = 1;
        while ai < 10000 {
            numbers.push(ai);
            ai = ai + 1;
        }
        let mut sum: i64 = 0;
        let mut mi: i64 = 0;
        while mi < 10000 {
            sum = sum + numbers[mi as usize];
            mi = mi + 1;
        }

        // 4. Nested loop (matrix multiply style)
        let mut matrix_sum: i64 = 0;
        let mut row: i64 = 0;
        while row < 200 {
            let mut col: i64 = 0;
            while col < 200 {
                matrix_sum = matrix_sum + row * col;
                col = col + 1;
            }
            row = row + 1;
        }
    }

    pub fn run(&mut self) {
        let start = Instant::now();
        let mut iter = 0;
        while iter < 100 {
            self.run_once();
            iter = iter + 1;
        }
        let elapsed = start.elapsed();
        println!("Rust (Varg-style): {:.2}ms", elapsed.as_secs_f64() * 1000.0);
    }
}

fn main() {
    let mut bench = Benchmark {};
    bench.run();
}
