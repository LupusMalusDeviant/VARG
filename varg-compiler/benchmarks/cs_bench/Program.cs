// Equivalent benchmark to benchmark_realistic.varg in C#
using System;

class BenchmarkRunner
{
    static int Fibonacci(int n)
    {
        int fa = 0, fb = 1;
        for (int i = 0; i < n; i++)
        {
            int temp = fb;
            fb = fa + fb;
            fa = temp;
        }
        return fa;
    }

    static int SumRange(int startVal, int endVal)
    {
        int total = 0;
        int idx = startVal;
        while (idx < endVal)
        {
            total += idx;
            idx++;
        }
        return total;
    }

    long counter = 0;

    void RunOnce()
    {
        // 1. Fibonacci(35)
        int fib = Fibonacci(35);
        counter += fib;

        // 2. String building (10000 concats)
        string result = "";
        for (int i = 0; i < 10000; i++)
        {
            result += "x";
        }

        // 3. Array fill + sum (10000 elements)
        var numbers = new System.Collections.Generic.List<int> { 0 };
        for (int i = 1; i < 10000; i++)
        {
            numbers.Add(i);
        }
        long sum = 0;
        for (int i = 0; i < 10000; i++)
        {
            sum += numbers[i];
        }

        // 4. Nested loop (matrix-like 200x200)
        long matrixSum = 0;
        for (int row = 0; row < 200; row++)
        {
            for (int col = 0; col < 200; col++)
            {
                matrixSum += row * col;
            }
        }

        // 5. Sum range
        int rangeSum = SumRange(0, 1000);
        counter += rangeSum;
    }

    void Run()
    {
        Console.WriteLine("=== C# Benchmark (100 iterations) ===");
        for (int i = 0; i < 100; i++)
        {
            RunOnce();
        }
        Console.WriteLine(counter);
        Console.WriteLine("=== Done ===");
    }

    static void Main()
    {
        new BenchmarkRunner().Run();
    }
}
