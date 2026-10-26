using System.Text;
using System.Text.Json;

Console.InputEncoding = new UTF8Encoding(false);
Console.OutputEncoding = new UTF8Encoding(false);
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
            "describe" => new { name = "CopyAll", version = "1.0.0", api = 1 },
            "scan" => Scan(request.GetProperty("params")),
            _ => throw new InvalidOperationException("Неизвестная команда.")
        };
        await Console.Out.WriteLineAsync(JsonSerializer.Serialize(new { jsonrpc = "2.0", id, result }));
    }
    catch(Exception ex) { await Console.Out.WriteLineAsync(JsonSerializer.Serialize(new { jsonrpc = "2.0", id, error = new { code = -32000, message = ex.Message } })); }
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
