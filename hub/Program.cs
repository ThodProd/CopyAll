using System.Text;
using System.Text.Json;



if(args.Length==1&&args[0]=="--trust-shell-certificate")
{
    try{ShellIntegration.TrustCertificate();}catch(Exception e){NativeClipboard.Error(e.Message);Environment.ExitCode=1;}return;
}
if(args.Length>=2&&args[0]=="--context-copy")
{
    try { var text=new StringBuilder();foreach(var path in args.Skip(1).Distinct(StringComparer.OrdinalIgnoreCase)){var result=CopyPaths(JsonSerializer.SerializeToElement(new{path}));var value=JsonSerializer.SerializeToElement(result);text.Append(value.GetProperty("clipboardText").GetString());if(text.Length>2_000_000)throw new IOException("Список превышает 2 МБ. Выберите меньше папок.");}NativeClipboard.Copy(text.ToString()); }
    catch(Exception e){NativeClipboard.Error(e.Message);Environment.ExitCode=1;}
    return;
}
if(args.Contains("--uninstall-context")||args.Contains("--register-integrations"))
{
    Console.OutputEncoding=new UTF8Encoding(false);
    try { if(args.Contains("--uninstall-context"))ContextMenu.Remove();else ContextMenu.Install();return; }
    catch(Exception e) { Console.Error.WriteLine(e.Message);Environment.ExitCode=1;return; }
}
Console.InputEncoding = new UTF8Encoding(false);
Console.OutputEncoding = new UTF8Encoding(false);
Task<object>? installJob=null;
object StartInstall(JsonElement request)
{
    if(installJob?.IsCompleted==false)return new{running=true};
    var consent=request.TryGetProperty("params",out var p)&&p.TryGetProperty("trustCertificate",out var trust)&&trust.GetBoolean();
    installJob=Task.Run(()=>ContextMenu.Install(consent));return new{running=true};
}
object InstallStatus()=>installJob is null?new{running=false}:!installJob.IsCompleted?new{running=true}:installJob.IsFaulted?new{running=false,error=installJob.Exception!.GetBaseException().Message}:new{running=false,state=installJob.Result};
while(await Console.In.ReadLineAsync() is { } line)
{
    string? id = null;
    try
    {
        using var document = JsonDocument.Parse(line);
        var request = document.RootElement;
        id = request.GetProperty("id").GetString();
        var method = request.GetProperty("method").GetString();
        object result = method switch
        {
            "describe" => new { name = "CopyAll", version = "1.1.2", api = 1 },
            "scan" => Scan(request.GetProperty("params")),
            "context.status" => ContextMenu.Status(),
            "context.install.start" => StartInstall(request),
            "context.install.status" => InstallStatus(),
            "context.install" => ContextMenu.Install(request.TryGetProperty("params",out var cp)&&cp.ValueKind==JsonValueKind.Object&&cp.TryGetProperty("trustCertificate",out var trust)&&trust.GetBoolean()),
            "context.remove" => ContextMenu.Remove(),
            "context.copy" => CopyPaths(request.GetProperty("params")),
            _ => throw new InvalidOperationException("Неизвестная команда.")
        };
        await Console.Out.WriteLineAsync(JsonSerializer.Serialize(new { jsonrpc = "2.0", id, result }));
    }
    catch(Exception ex) { await Console.Out.WriteLineAsync(JsonSerializer.Serialize(new { jsonrpc = "2.0", id, error = new { code = -32000, message = ex.Message } })); }
}

static object CopyPaths(JsonElement p)
{
    var path=Path.GetFullPath(p.GetProperty("path").GetString()??"");
    if(File.Exists(path))return new{clipboardText=path+Environment.NewLine,skipped=0};
    var input=JsonSerializer.SerializeToElement(new { path=p.GetProperty("path").GetString(),mode="full",includeHidden=true });
    var result=JsonSerializer.SerializeToElement(Scan(input));
    return new { clipboardText=result.GetProperty("text").GetString(),skipped=result.GetProperty("skipped").GetInt32() };
}

static object Scan(JsonElement p)
{
    var path = Path.GetFullPath(p.GetProperty("path").GetString() ?? "");
    if(!Directory.Exists(path)) throw new DirectoryNotFoundException("Выбранная папка не найдена.");
    var mode = p.TryGetProperty("mode", out var m) ? m.GetString() : "tree";
    var includeHidden = p.TryGetProperty("includeHidden", out var h) && h.GetBoolean();
    int files = 0, folders = 0, skipped = 0;
    var text = new StringBuilder().AppendLine(path);
    var stack = new Stack<(string Path, int Depth)>(); stack.Push((path, 0));
    while(stack.TryPop(out var entry))
    {
        if(files + folders >= 20000 || text.Length > 2_000_000) throw new InvalidOperationException("Папка слишком большая (предел 20 000 элементов / 2 МБ). Выберите подпапку.");
        if(entry.Depth > 64) { skipped++; continue; }
        try
        {
            if(entry.Depth > 0 && mode == "tree") text.Append(' ', entry.Depth * 2).AppendLine(Path.GetFileName(entry.Path) + "/");
            var children = new DirectoryInfo(entry.Path).EnumerateFileSystemInfos().OrderBy(x => x.Name, StringComparer.OrdinalIgnoreCase).ToArray();
            var subfolders = new List<string>();
            foreach(var child in children)
            {
                if((child.Attributes & FileAttributes.ReparsePoint) != 0) { skipped++; continue; }
                if(!includeHidden && (child.Attributes & (FileAttributes.Hidden|FileAttributes.System)) != 0) { skipped++; continue; }
                if((child.Attributes & FileAttributes.Directory) != 0)
                {
                    folders++;subfolders.Add(child.FullName);
                }
                else
                {
                    files++;
                    if(mode == "tree") text.Append(' ', (entry.Depth + 1) * 2);
                    text.AppendLine(mode == "relative" ? Path.GetRelativePath(path, child.FullName) : child.FullName);
                }
                if(files + folders >= 20000 || text.Length > 2_000_000) throw new InvalidOperationException("Папка слишком большая (предел 20 000 элементов / 2 МБ). Выберите подпапку.");
            }
            for(var i = subfolders.Count - 1; i >= 0; i--) stack.Push((subfolders[i], entry.Depth + 1));
        }
        catch(UnauthorizedAccessException) { skipped++; }
        catch(IOException) { skipped++; }
    }
    return new { text = text.ToString(), files, folders, skipped };
}
