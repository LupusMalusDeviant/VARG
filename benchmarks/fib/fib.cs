using System.Diagnostics;

static int Fib(int n)
{
    if (n <= 1) return n;
    return Fib(n - 1) + Fib(n - 2);
}

var sw = Stopwatch.StartNew();
var result = Fib(35);
sw.Stop();
Console.WriteLine($"fib(35) = {result}");
Console.WriteLine($"Time: {sw.ElapsedMilliseconds}ms");
