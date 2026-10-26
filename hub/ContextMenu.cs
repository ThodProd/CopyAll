using Microsoft.Win32;
using System.Runtime.InteropServices;

internal static class ContextMenu
{
    private const string Owner="ThodHUB.CopyAll.v1";
    private static readonly string[] Keys=[@"Software\Classes\Directory\shell\ThodHUB.CopyAll",@"Software\Classes\Directory\Background\shell\ThodHUB.CopyAll"];
    public static bool IsInstalled()=>Keys.All(path=>{using var key=Registry.CurrentUser.OpenSubKey(path);return key?.GetValue("ThodOwner") as string==Owner;});
    public static object Install()
    {
        var launcher=Environment.GetEnvironmentVariable("THODHUB_LAUNCHER");
        if(string.IsNullOrWhiteSpace(launcher)||!File.Exists(launcher))throw new InvalidOperationException("Установите пункт меню через запущенный ThodHUB.");
        foreach(var path in Keys) {using var existing=Registry.CurrentUser.OpenSubKey(path);if(existing is not null&&existing.GetValue("ThodOwner") as string!=Owner)throw new IOException("Ключ меню занят другой программой.");}
        foreach(var path in Keys)
        {
            using var key=Registry.CurrentUser.CreateSubKey(path);
            key.SetValue("","CopyAll — копировать все пути");key.SetValue("ThodOwner",Owner);key.SetValue("Icon","\""+launcher+"\",0");
            using var command=key.CreateSubKey("command");
            command.SetValue("","\""+launcher+"\" --invoke copyall context.copy \""+(path.Contains("Background")?"%V":"%1")+"\"");
        }
        Notify();return new {installed=true};
    }
    public static object Remove()
    {
        foreach(var path in Keys)
        {
            bool owned;using(var key=Registry.CurrentUser.OpenSubKey(path)){owned=key?.GetValue("ThodOwner") as string==Owner;}
            if(owned)Registry.CurrentUser.DeleteSubKeyTree(path,false);
        }
        Notify();return new {installed=false};
    }
    private static void Notify()=>SHChangeNotify(0x08000000,0,IntPtr.Zero,IntPtr.Zero);
    [DllImport("shell32.dll")] private static extern void SHChangeNotify(uint eventId,uint flags,IntPtr item1,IntPtr item2);
}
