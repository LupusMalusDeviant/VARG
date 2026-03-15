// Equivalent benchmark to benchmark_realistic.varg in TypeScript

function fibonacci(n: number): number {
    let fa = 0, fb = 1;
    for (let i = 0; i < n; i++) {
        const temp = fb;
        fb = fa + fb;
        fa = temp;
    }
    return fa;
}

function sumRange(startVal: number, endVal: number): number {
    let total = 0;
    let idx = startVal;
    while (idx < endVal) {
        total += idx;
        idx += 1;
    }
    return total;
}

class Benchmark {
    counter = 0;

    runOnce(): void {
        // 1. Fibonacci(35)
        const fib = fibonacci(35);
        this.counter += fib;

        // 2. String building (10000 concats)
        let result = "";
        for (let i = 0; i < 10000; i++) {
            result += "x";
        }

        // 3. Array fill + sum (10000 elements)
        const numbers = [0];
        for (let i = 1; i < 10000; i++) {
            numbers.push(i);
        }
        let sum = 0;
        for (let i = 0; i < 10000; i++) {
            sum += numbers[i];
        }

        // 4. Nested loop (matrix-like 200x200)
        let matrixSum = 0;
        for (let row = 0; row < 200; row++) {
            for (let col = 0; col < 200; col++) {
                matrixSum += row * col;
            }
        }

        // 5. Sum range
        const rangeSum = sumRange(0, 1000);
        this.counter += rangeSum;
    }

    run(): void {
        console.log("=== TypeScript/Bun Benchmark (100 iterations) ===");
        for (let i = 0; i < 100; i++) {
            this.runOnce();
        }
        console.log(this.counter);
        console.log("=== Done ===");
    }
}

new Benchmark().run();
