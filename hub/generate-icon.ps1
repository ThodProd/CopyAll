$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Drawing
$destination=Join-Path $PSScriptRoot 'assets'
New-Item -ItemType Directory -Force -Path $destination | Out-Null
$images=@()
foreach($size in @(16,24,32,48,64,128,256)) {
    $bitmap=[Drawing.Bitmap]::new($size,$size)
    $graphics=[Drawing.Graphics]::FromImage($bitmap)
    $graphics.SmoothingMode='AntiAlias'
    $graphics.Clear([Drawing.Color]::FromArgb(25,155,168))
    $brush=[Drawing.SolidBrush]::new([Drawing.Color]::White)
    $unit=$size/8.0
    $pen=[Drawing.Pen]::new([Drawing.Color]::White,[single]($size/16));$graphics.DrawRectangle($pen,[single]($unit*1.5),[single]($unit*1.5),[single]($unit*3.2),[single]($unit*3.6));$graphics.FillRectangle([Drawing.Brushes]::Teal,[single]($unit*3),[single]($unit*3),[single]($unit*3.2),[single]($unit*3.6));$graphics.DrawRectangle($pen,[single]($unit*3),[single]($unit*3),[single]($unit*3.2),[single]($unit*3.6));$pen.Dispose()
    $memory=[IO.MemoryStream]::new();$bitmap.Save($memory,[Drawing.Imaging.ImageFormat]::Png)
    $images+=,@{Size=$size;Bytes=$memory.ToArray()};if($size-eq 256){[IO.File]::WriteAllBytes((Join-Path $destination 'Logo.png'),$memory.ToArray())}
    $memory.Dispose();$graphics.Dispose();$bitmap.Dispose();$brush.Dispose()
}
$file=[IO.File]::Create((Join-Path $destination 'copyall.ico'));$writer=[IO.BinaryWriter]::new($file)
try {
    $writer.Write([uint16]0);$writer.Write([uint16]1);$writer.Write([uint16]$images.Count)
    $offset=6+16*$images.Count
    foreach($image in $images){$s=if($image.Size-eq 256){0}else{$image.Size};$writer.Write([byte]$s);$writer.Write([byte]$s);$writer.Write([uint16]0);$writer.Write([uint16]1);$writer.Write([uint16]32);$writer.Write([uint32]$image.Bytes.Length);$writer.Write([uint32]$offset);$offset+=$image.Bytes.Length}
    foreach($image in $images){$writer.Write([byte[]]$image.Bytes)}
} finally {$writer.Dispose();$file.Dispose()}
