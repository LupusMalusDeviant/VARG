// === TypeScript Realistic Benchmark (equivalent) ===

interface Summable {
    getTotal(): number;
}

function fibonacci(n: number): number {
    let a = 0;
    let b = 1;
    for (let i = 0; i < n; i++) {
        const temp = b;
        b = a + b;
        a = temp;
    }
    return a;
}

function sumRange(from: number, to: number): number {
    let total = 0;
    for (let i = from; i < to; i++) {
        total += i;
    }
    return total;
}

class Calculator implements Summable {
    private result: number = 0;

    getTotal(): number {
        return this.result;
    }

    computeFibs(count: number): void {
        for (let i = 0; i < count; i++) {
            this.result += fibonacci(30);
        }
    }

    buildReport(items: number): string {
        let report = "";
        for (let i = 0; i < items; i++) {
            report += "item ";
        }
        return report;
    }

    arrayWork(size: number): number {
        const numbers: number[] = [0];
        for (let i = 1; i < size; i++) {
            numbers.push(i * 2);
        }
        let sum = 0;
        for (let j = 0; j < size; j++) {
            sum += numbers[j];
        }
        return sum;
    }

    run(): void {
        console.log("=== TypeScript Realistic Benchmark ===");

        // 1. Standalone function calls
        const fib35 = fibonacci(35);
        console.log(fib35);

        const rangeSum = sumRange(0, 100000);
        console.log(rangeSum);

        // 2. Class method calls
        this.computeFibs(50);
        console.log(this.result);

        // 3. String building (5000 concats)
        const report = this.buildReport(5000);
        console.log(report.length);

        // 4. Array fill + sum (10000 elements)
        const arrSum = this.arrayWork(10000);
        console.log(arrSum);

        // 5. Nested loop (matrix-like)
        let matrixSum = 0;
        for (let row = 0; row < 100; row++) {
            for (let col = 0; col < 100; col++) {
                matrixSum += row * col;
            }
        }
        console.log(matrixSum);

        console.log("=== Done ===");
    }
}

const start = performance.now();
const calc = new Calculator();
calc.run();
const end = performance.now();
console.log(`Time: ${(end - start).toFixed(2)}ms`);
