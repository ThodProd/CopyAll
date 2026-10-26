using Microsoft.Win32;
using System;
using System.Diagnostics;
using System.IO;
using System.Text;
using System.Windows;

namespace CopyAll
{
    public partial class App : Application
    {
        private const string MenuKey = @"Directory\shell\CopyNestedPaths";
        private const string MenuText = "Скопировать путь всех вложенных файлов";

        protected override void OnStartup(StartupEventArgs e)
        {
            base.OnStartup(e);

            // Если пришли аргументы → это вызов из контекстного меню
            if (e.Args.Length > 0)
            {
                string path = e.Args[0].Trim('"').Trim();

                // Проверяем, что это реальный путь, а не какой-то флаг
                if (!string.IsNullOrWhiteSpace(path) &&
                    (Directory.Exists(path) || File.Exists(path)))
                {
                    // Для файлов пока ничего не делаем, только для папок
                    if (Directory.Exists(path))
                    {
                        ProcessFolder(path);
                    }
                    else
                    {
                        MessageBox.Show("Выбран файл, а не папка.\nФункция работает только для папок.",
                            "Информация", MessageBoxButton.OK, MessageBoxImage.Information);
                    }
                }
                else
                {
                    MessageBox.Show("Некорректный путь.", "Ошибка", MessageBoxButton.OK, MessageBoxImage.Error);
                }

                // Важно: завершаем приложение после обработки
                Shutdown();
                return;
            }

            // Нет аргументов → обычный запуск → открывается MainWindow
            // (StartupUri уже указывает на MainWindow.xaml)
        }

        private static void ProcessFolder(string folderPath)
        {
            try
            {
                string treeText = BuildFolderTree(folderPath, 0);

                // Копируем в буфер обмена
                Clipboard.SetText(treeText);

                // Создаём временный файл с понятным именем
                string safeName = DateTime.Now.ToString("yyyy-MM-dd_HH-mm-ss");
                string tempFile = Path.Combine(Path.GetTempPath(), $"Структура_{safeName}.txt");

                File.WriteAllText(tempFile, treeText, Encoding.UTF8);

                // Открываем файл в программе по умолчанию (обычно Блокнот)
                Process.Start(new ProcessStartInfo
                {
                    FileName = tempFile,
                    UseShellExecute = true
                });
            }
            catch (Exception ex)
            {
                MessageBox.Show($"Не удалось создать список файлов:\n{ex.Message}",
                    "Ошибка", MessageBoxButton.OK, MessageBoxImage.Error);
            }
        }

        private static string BuildFolderTree(string rootPath, int level = 0)
        {
            var sb = new StringBuilder();

            // На самом верхнем уровне показываем полный путь к корневой папке
            if (level == 0)
            {
                sb.AppendLine(rootPath + "/");
            }
            else
            {
                string indent = new string(' ', level * 4);
                string name = Path.GetFileName(rootPath);
                sb.AppendLine($"{indent}{name}/");
            }

            try
            {
                // Файлы — полный путь
                foreach (string file in Directory.GetFiles(rootPath))
                {
                    string indent = new string(' ', (level + 1) * 4);
                    sb.AppendLine($"{indent}{file}");
                }

                // Папки — рекурсия
                foreach (string dir in Directory.GetDirectories(rootPath))
                {
                    sb.Append(BuildFolderTree(dir, level + 1));
                }
            }
            catch (UnauthorizedAccessException)
            {
                string indent = new string(' ', (level + 1) * 4);
                sb.AppendLine($"{indent}[нет доступа]");
            }
            catch (Exception ex)
            {
                string indent = new string(' ', (level + 1) * 4);
                sb.AppendLine($"{indent}[ошибка: {ex.Message}]");
            }

            return sb.ToString();
        }

        public static void InstallContextMenu()
        {
            try
            {
                using var key = Registry.ClassesRoot.CreateSubKey(MenuKey);
                key.SetValue("", MenuText);

                using var command = key.CreateSubKey("command");
                string exePath = Process.GetCurrentProcess().MainModule!.FileName;
                command.SetValue("", $"\"{exePath}\" \"%1\"");

                MessageBox.Show("Пункт меню успешно добавлен.", "Успех", MessageBoxButton.OK, MessageBoxImage.Information);
            }
            catch (Exception ex)
            {
                MessageBox.Show($"Не удалось добавить пункт меню.\n{ex.Message}\n\nЗапустите программу от имени администратора.",
                    "Ошибка", MessageBoxButton.OK, MessageBoxImage.Error);
            }
        }

        public static void RemoveContextMenu()
        {
            try
            {
                Registry.ClassesRoot.DeleteSubKeyTree(MenuKey, false);
                MessageBox.Show("Пункт меню успешно удалён.", "Успех", MessageBoxButton.OK, MessageBoxImage.Information);
            }
            catch (Exception ex)
            {
                MessageBox.Show($"Не удалось удалить пункт меню.\n{ex.Message}\n\nЗапустите программу от имени администратора.",
                    "Ошибка", MessageBoxButton.OK, MessageBoxImage.Error);
            }
        }
    }
}