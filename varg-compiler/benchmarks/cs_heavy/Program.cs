// Heavy benchmark equivalent in C#
using System;

class HeavyBench
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
            idx += 1;
        }
        return total;
    }

    long counter = 0;

    void RunOnce()
    {
        int fib = Fibonacci(40);
        counter += fib;

        long matrixSum = 0;
        for (int row = 0; row < 300; row++)
        {
            for (int col = 0; col < 300; col++)
            {
                matrixSum += row * col;
            }
        }

        int rangeSum = SumRange(0, 50000);
        counter += rangeSum;
    }

    void Run()
    {
        Console.WriteLine("=== C# Heavy Benchmark (1000 iter) ===");
        for (int i = 0; i < 1000; i++)
        {
            RunOnce();
        }
        Console.WriteLine(counter);
        Console.WriteLine("=== Done ===");
    }

    static void Main()
    {
        new HeavyBench().Run();
    }
}
