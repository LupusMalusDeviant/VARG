// TypeScript equivalent benchmark
class Benchmark {
    runOnce(): void {
        // 1. Fibonacci(35)
        const n = 35;
        let fa = 0;
        let fb = 1;
        let fi = 0;
        while (fi < n) {
            const temp = fb;
            fb = fa + fb;
            fa = temp;
            fi = fi + 1;
        }

        // 2. String building (10000 concats)
        let result = "";
        let si = 0;
        while (si < 10000) {
            result += "x";
            si = si + 1;
        }

        // 3. Array fill + sum (10000 elements)
        const numbers: number[] = [0];
        let ai = 1;
        while (ai < 10000) {
            numbers.push(ai);
            ai = ai + 1;
        }
        let sum = 0;
        let mi = 0;
        while (mi < 10000) {
            sum = sum + numbers[mi];
            mi = mi + 1;
        }

        // 4. Nested loop (matrix multiply style)
        let matrixSum = 0;
        let row = 0;
        while (row < 200) {
            let col = 0;
            while (col < 200) {
                matrixSum = matrixSum + row * col;
                col = col + 1;
            }
            row = row + 1;
        }
    }

    run(): void {
        const start = performance.now();
        let iter = 0;
        while (iter < 100) {
            this.runOnce();
            iter = iter + 1;
        }
        const elapsed = performance.now() - start;
        console.log(`TypeScript (Node.js): ${elapsed.toFixed(2)}ms`);
    }
}

const bench = new Benchmark();
bench.run();
