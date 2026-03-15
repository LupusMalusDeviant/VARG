// Heavy benchmark equivalent
function fibonacci(n) {
    let fa = 0, fb = 1;
    for (let i = 0; i < n; i++) {
        const temp = fb;
        fb = fa + fb;
        fa = temp;
    }
    return fa;
}

function sumRange(startVal, endVal) {
    let total = 0;
    let idx = startVal;
    while (idx < endVal) {
        total += idx;
        idx += 1;
    }
    return total;
}

class HeavyBench {
    constructor() { this.counter = 0; }

    runOnce() {
        const fib = fibonacci(40);
        this.counter += fib;

        let matrixSum = 0;
        for (let row = 0; row < 300; row++) {
            for (let col = 0; col < 300; col++) {
                matrixSum += row * col;
            }
        }

        const rangeSum = sumRange(0, 50000);
        this.counter += rangeSum;
    }

    run() {
        console.log("=== Node Heavy Benchmark (1000 iter) ===");
        for (let i = 0; i < 1000; i++) {
            this.runOnce();
        }
        console.log(this.counter);
        console.log("=== Done ===");
    }
}

new HeavyBench().run();
