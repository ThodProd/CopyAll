using System.Diagnostics;
using System.Security.Cryptography.X509Certificates;
using System.Text;

internal static class ShellIntegration
{
    private const string Thumbprint="0E85D468BD75DF47E5B93DBE77C5141A2B6BCFED";
    public static bool Supported=>OperatingSystem.IsWindowsVersionAtLeast(10,0,22000);
    private static string Quote(string value)=>"'"+value.Replace("'","''")+"'";
    private static string Run(string command)
    {
        var psi=new ProcessStartInfo(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.System),@"WindowsPowerShell\v1.0\powershell.exe")){UseShellExecute=false,CreateNoWindow=true,RedirectStandardOutput=true,RedirectStandardError=true};
        foreach(var a in new[]{"-NoLogo","-NoProfile","-NonInteractive","-OutputFormat","Text","-EncodedCommand",Convert.ToBase64String(Encoding.Unicode.GetBytes("$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue';[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false);try{"+command+"}catch{[Console]::Error.WriteLine($_.Exception.Message);exit 1}"))})psi.ArgumentList.Add(a);
        psi.StandardOutputEncoding=psi.StandardErrorEncoding=Encoding.UTF8;
        using var process=Process.Start(psi)??throw new IOException("Не удалось запустить установку расширения Проводника.");
        var output=process.StandardOutput.ReadToEndAsync();var error=process.StandardError.ReadToEndAsync();
        if(!process.WaitForExit(25000)){process.Kill(true);throw new IOException("Windows не завершила регистрацию меню за 25 секунд. Повторите установку.");}
        Task.WaitAll(output,error);if(process.ExitCode!=0)throw new IOException("Расширение Проводника: "+error.Result.Trim());return output.Result.Trim();
    }
    public static bool Trusted()
    {
        using var store=new X509Store(StoreName.TrustedPeople,StoreLocation.LocalMachine);store.Open(OpenFlags.ReadOnly);
        return store.Certificates.Find(X509FindType.FindByThumbprint,Thumbprint,false).Count>0;
    }
    public static bool Installed()=>Supported&&Run("$p=Get-AppxPackage -Name ThodProd.CopyAll;if($p){$p.Version.ToString()}")==typeof(ShellIntegration).Assembly.GetName().Version!.ToString();
    public static void TrustCertificate()
    {
        using var cert=X509CertificateLoader.LoadCertificateFromFile(Path.Combine(AppContext.BaseDirectory,"shell","CopyAll.cer"));
        if(cert.Thumbprint!=Thumbprint)throw new IOException("Сертификат расширения не совпадает с сертификатом CopyAll.");
        using var store=new X509Store(StoreName.TrustedPeople,StoreLocation.LocalMachine);store.Open(OpenFlags.ReadWrite);store.Add(cert);
    }
    public static void Install(bool allowTrust=false)
    {
        if(!Supported)return;
        if(!Trusted())
        {
            if(!allowTrust)throw new IOException("Для первого меню Windows 11 откройте CopyAll и подтвердите установку расширения. Классическое меню уже доступно.");
            var info=new ProcessStartInfo(Environment.ProcessPath!){UseShellExecute=true,Verb="runas",WindowStyle=ProcessWindowStyle.Hidden};info.ArgumentList.Add("--trust-shell-certificate");
            using var process=Process.Start(info)??throw new IOException("Доверие сертификату не подтверждено.");
            if(!process.WaitForExit(120000)||process.ExitCode!=0||!Trusted())throw new IOException("Windows не подтвердила доверие сертификату CopyAll.");
        }
        var package=Path.Combine(AppContext.BaseDirectory,"shell","CopyAll.msix");
        // The worker itself stays unpackaged (no msix fusion identity in app.manifest).
        // Otherwise Windows waits for this very installer process to exit during removal.
        // Re-register also repairs the external location after moving/updating a portable hub.
        Run("$p=Get-AppxPackage -Name ThodProd.CopyAll;if($p){Remove-AppxPackage -Package $p.PackageFullName};Add-AppxPackage -Path "+Quote(package)+" -ExternalLocation "+Quote(AppContext.BaseDirectory));
    }
    public static void Remove()
    {
        if(Supported)Run("Get-AppxPackage -Name ThodProd.CopyAll | Remove-AppxPackage");
        // Keep the explicitly trusted publisher for future reinstalls. Never modify root trust.
    }
}
