using System.Diagnostics;
using VargWebIde.Models;

namespace VargWebIde.Services;

public enum BuildTarget { Linux, Windows }

public class VargCompilerService
{
    private readonly string _vargcPath;
    private readonly string _cacheDir;
    private readonly string? _cratesDir;

    // Serialise builds: shared cargo target dir would race if two builds run simultaneously.
    private static readonly SemaphoreSlim _buildSem = new(1, 1);

    public VargCompilerService(IConfiguration config)
    {
        _vargcPath = string.IsNullOrEmpty(config["Varg:VargcPath"]) ? "vargc" : config["Varg:VargcPath"]!;
        _cacheDir = string.IsNullOrEmpty(config["Varg:BuildCache"])
            ? Path.Combine(Path.GetTempPath(), "varg-playground-cache")
            : config["Varg:BuildCache"]!;
        Directory.CreateDirectory(_cacheDir);

        var cratesDir = config["Varg:CratesDir"];
        _cratesDir = string.IsNullOrEmpty(cratesDir) ? null : cratesDir;
    }

    // files: list of (filename, content). main.varg must be present; other files are written
    // alongside so vargc can resolve them (or they get concatenated by the caller).
    public Task<EmitResult> EmitRsAsync(IReadOnlyList<(string Name, string Code)> files) =>
        Task.Run(() => RunEmitRs(files));

    public Task<BuildResult> BuildAsync(
        IReadOnlyList<(string Name, string Code)> files,
        BuildTarget target = BuildTarget.Linux) =>
        Task.Run(() => RunBuild(files, target));

    // ── emit-rs ──────────────────────────────────────────────────────────────

    private EmitResult RunEmitRs(IReadOnlyList<(string Name, string Code)> files)
    {
        using var tmp = WriteTempDir(files);
        var (combined, success) = RunProcess(_vargcPath, ["emit-rs", "main.varg"], tmp.Path);
        if (!success)
            return new EmitResult { Success = false, Output = combined };

        var rsPath = Path.Combine(tmp.Path, "main.rs");
        var rustSource = File.Exists(rsPath) ? File.ReadAllText(rsPath) : "(Rust source not found)";
        return new EmitResult { Success = true, Output = combined, RustSource = rustSource };
    }

    // ── build ─────────────────────────────────────────────────────────────────

    private BuildResult RunBuild(IReadOnlyList<(string Name, string Code)> files, BuildTarget target)
    {
        _buildSem.Wait();
        try
        {
            using var tmp = WriteTempDir(files);
            var env = new Dictionary<string, string> { ["CARGO_TARGET_DIR"] = _cacheDir };
            if (target == BuildTarget.Windows)
            {
                env["VARGC_TARGET_TRIPLE"] = "x86_64-pc-windows-gnu";
                env["CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER"] = "x86_64-w64-mingw32-gcc";
                // Statically link MinGW runtime so the .exe has no external DLL dependencies
                env["RUSTFLAGS"] = "-C target-feature=+crt-static -C link-arg=-static";
            }
            var (combined, success) = RunProcess(_vargcPath, ["build", "main.varg"], tmp.Path, env);

            if (!success)
                return new BuildResult { Success = false, Output = combined };

            var exeName = target == BuildTarget.Windows ? "main.exe" : "main";
            var exePath = Path.Combine(tmp.Path, exeName);

            if (!File.Exists(exePath))
                return new BuildResult { Success = false, Output = $"{combined}\nBinary not found: {exePath}" };

            return new BuildResult { Success = true, Output = combined, Binary = File.ReadAllBytes(exePath) };
        }
        finally
        {
            _buildSem.Release();
        }
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    private TempDir WriteTempDir(IReadOnlyList<(string Name, string Code)> files)
    {
        var dir = Directory.CreateTempSubdirectory("varg-play-");

        foreach (var (name, code) in files)
            File.WriteAllText(Path.Combine(dir.FullName, name), code);

        if (_cratesDir is not null && Directory.Exists(_cratesDir))
        {
            try { Directory.CreateSymbolicLink(Path.Combine(dir.FullName, "crates"), _cratesDir); }
            catch { }
        }

        return new TempDir(dir.FullName);
    }

    private static (string output, bool success) RunProcess(
        string exe, string[] args, string workDir,
        Dictionary<string, string>? env = null)
    {
        var psi = new ProcessStartInfo(exe)
        {
            WorkingDirectory = workDir,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
            CreateNoWindow = true,
        };
        foreach (var a in args) psi.ArgumentList.Add(a);
        if (env is not null)
            foreach (var (k, v) in env)
                psi.Environment[k] = v;

        try
        {
            using var p = Process.Start(psi)!;
            var stdout = p.StandardOutput.ReadToEnd();
            var stderr = p.StandardError.ReadToEnd();
            p.WaitForExit();
            return (stdout + stderr, p.ExitCode == 0);
        }
        catch (Exception ex)
        {
            return ($"vargc not found: {ex.Message}\nCheck VARGC_PATH or add vargc to PATH.", false);
        }
    }

    private sealed class TempDir(string path) : IDisposable
    {
        public string Path { get; } = path;
        public void Dispose() { try { Directory.Delete(Path, recursive: true); } catch { } }
    }
}
