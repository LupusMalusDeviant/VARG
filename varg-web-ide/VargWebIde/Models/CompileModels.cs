namespace VargWebIde.Models;

public class EmitResult
{
    public bool Success { get; set; }
    public string Output { get; set; } = "";
    public string RustSource { get; set; } = "";
}

public class BuildResult
{
    public bool Success { get; set; }
    public string Output { get; set; } = "";
    public byte[]? Binary { get; set; }
}
