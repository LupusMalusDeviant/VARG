using System.Diagnostics;
using VargWebIde.Models;

namespace VargWebIde.Services;

public class VargCompilerService
{
    private readonly string _vargcPath;
    private readonly string _cacheDir;
    private readonly string? _cratesDir;

    public VargCompilerService(IConfiguration config)
    {
        _vargcPath = string.IsNullOrEmpty(config["Varg:VargcPath"]) ? "vargc" : config["Varg:VargcPath"]!;
        _cacheDir = string.IsNullOrEmpty(config["Varg:BuildCache"])
            ? Path.Combine(Path.GetTempPath(), "varg-playground-cache")
            : config["Varg:BuildCache"]!;
        Directory.CreateDirectory(_cacheDir);

        // Optional: directory containing varg-os-types/ and varg-runtime/ subdirs.
        // vargc resolves path deps relative to its cwd, so we symlink this into
        // every temp build dir so cargo can find them.
        var cratesDir = config["Varg:CratesDir"];
        _cratesDir = string.IsNullOrEmpty(cratesDir) ? null : cratesDir;
    }

    public Task<EmitResult> EmitRsAsync(string code) =>
        Task.Run(() => RunEmitRs(code));

    public Task<BuildResult> BuildAsync(string code) =>
        Task.Run(() => RunBuild(code));

    // ── emit-rs ──────────────────────────────────────────────────────────────

    private EmitResult RunEmitRs(string code)
    {
        using var tmp = WriteTempDir(code);
        var (combined, success) = RunProcess(_vargcPath, ["emit-rs", "main.varg"], tmp.Path);
        if (!success)
            return new EmitResult { Success = false, Output = combined };

        var rsPath = Path.Combine(tmp.Path, "main.rs");
        var rustSource = File.Exists(rsPath)
            ? File.ReadAllText(rsPath)
            : "(Rust source not found)";

        return new EmitResult { Success = true, Output = combined, RustSource = rustSource };
    }

    // ── build ─────────────────────────────────────────────────────────────────

    private BuildResult RunBuild(string code)
    {
        using var tmp = WriteTempDir(code);

        var env = new Dictionary<string, string> { ["CARGO_TARGET_DIR"] = _cacheDir };
        var (combined, success) = RunProcess(_vargcPath, ["build", "main.varg"], tmp.Path, env);

        if (!success)
            return new BuildResult { Success = false, Output = combined };

        var exeName = OperatingSystem.IsWindows() ? "main.exe" : "main";
        var exePath = Path.Combine(tmp.Path, exeName);

        if (!File.Exists(exePath))
            return new BuildResult { Success = false, Output = $"{combined}\nBinary not found: {exePath}" };

        return new BuildResult
        {
            Success = true,
            Output = combined,
            Binary = File.ReadAllBytes(exePath)
        };
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    private TempDir WriteTempDir(string code)
    {
        var dir = Directory.CreateTempSubdirectory("varg-play-");
        File.WriteAllText(Path.Combine(dir.FullName, "main.varg"), code);

        // Symlink the varg crates so vargc can resolve path dependencies.
        // vargc computes: current_dir + "crates/varg-os-types" etc.
        if (_cratesDir is not null && Directory.Exists(_cratesDir))
        {
            try
            {
                Directory.CreateSymbolicLink(
                    Path.Combine(dir.FullName, "crates"),
                    _cratesDir);
            }
            catch { /* symlinks may require elevated perms on Windows; ignore */ }
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
