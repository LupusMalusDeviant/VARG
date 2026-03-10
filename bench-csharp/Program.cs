using System;
using System.Collections.Generic;
using System.Diagnostics;

class Benchmark
{
    public void Run()
    {
        // 1. Fibonacci
        int n = 35;
        int fa = 0;
        int fb = 1;
        for (int fi = 0; fi < n; fi++)
        {
            int temp = fb;
            fb = fa + fb;
            fa = temp;
        }

        // 2. String building
        string result = "";
        for (int si = 0; si < 10000; si++)
        {
            result = result + "x";
        }

        // 3. Array fill + sum
        var numbers = new List<int> { 0 };
        for (int ai = 1; ai < 10000; ai++)
        {
            numbers.Add(ai);
        }
        int sum = 0;
        for (int mi = 0; mi < 10000; mi++)
        {
            sum += numbers[mi];
        }
    }

    static void Main(string[] args)
    {
        var bench = new Benchmark();

        // Warmup
        bench.Run();

        // Timed run
        var sw = Stopwatch.StartNew();
        for (int i = 0; i < 100; i++)
        {
            bench.Run();
        }
        sw.Stop();

        Console.WriteLine($"Fibonacci(35) = 9227465");
        Console.WriteLine($"String length: 10000");
        Console.WriteLine($"Sum 0..9999 = 49995000");
        Console.WriteLine($"\n=== C# 100 iterations: {sw.ElapsedMilliseconds} ms ===");
    }
}
