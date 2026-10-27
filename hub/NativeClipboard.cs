using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;

internal static class NativeClipboard
{
    public static void Copy(string text)
    {
        var window=CreateWindowEx(0,"STATIC","CopyAll",0,0,0,0,0,new IntPtr(-3),IntPtr.Zero,IntPtr.Zero,IntPtr.Zero);
        if(window==IntPtr.Zero)throw new Win32Exception(Marshal.GetLastWin32Error());
        IntPtr memory=IntPtr.Zero;bool opened=false;
        try
        {
            for(var i=0;i<20;i++){if(OpenClipboard(window)){opened=true;break;}Thread.Sleep(50);}
            if(!opened)throw new IOException("Буфер обмена занят другой программой. Повторите копирование.");
            var bytes=Encoding.Unicode.GetBytes(text+'\0');memory=GlobalAlloc(0x0002,(nuint)bytes.Length);
            if(memory==IntPtr.Zero)throw new OutOfMemoryException();
            var pointer=GlobalLock(memory);if(pointer==IntPtr.Zero)throw new Win32Exception(Marshal.GetLastWin32Error());
            try{Marshal.Copy(bytes,0,pointer,bytes.Length);}finally{GlobalUnlock(memory);}
            if(!EmptyClipboard()||SetClipboardData(13,memory)==IntPtr.Zero)throw new Win32Exception(Marshal.GetLastWin32Error());
            memory=IntPtr.Zero;
        }
        finally{if(memory!=IntPtr.Zero)GlobalFree(memory);if(opened)CloseClipboard();DestroyWindow(window);}
    }
    public static void Error(string text)=>MessageBox(IntPtr.Zero,text,"CopyAll",0x10);
    [DllImport("user32.dll",CharSet=CharSet.Unicode,SetLastError=true)]private static extern IntPtr CreateWindowEx(uint ex,string cls,string name,uint style,int x,int y,int w,int h,IntPtr parent,IntPtr menu,IntPtr instance,IntPtr param);
    [DllImport("user32.dll")]private static extern bool DestroyWindow(IntPtr window);
    [DllImport("user32.dll",SetLastError=true)]private static extern bool OpenClipboard(IntPtr window);
    [DllImport("user32.dll")]private static extern bool CloseClipboard();
    [DllImport("user32.dll",SetLastError=true)]private static extern bool EmptyClipboard();
    [DllImport("user32.dll",SetLastError=true)]private static extern IntPtr SetClipboardData(uint format,IntPtr data);
    [DllImport("kernel32.dll",SetLastError=true)]private static extern IntPtr GlobalAlloc(uint flags,nuint bytes);
    [DllImport("kernel32.dll",SetLastError=true)]private static extern IntPtr GlobalLock(IntPtr memory);
    [DllImport("kernel32.dll")]private static extern bool GlobalUnlock(IntPtr memory);
    [DllImport("kernel32.dll")]private static extern IntPtr GlobalFree(IntPtr memory);
    [DllImport("user32.dll",CharSet=CharSet.Unicode)]private static extern int MessageBox(IntPtr window,string text,string title,uint flags);
}
